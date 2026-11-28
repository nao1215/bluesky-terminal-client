//! A loopback HTTP stand-in for client unit tests, with byte-preserving bodies.
use std::{
    io::{self, BufRead, BufReader, Read, Write},
    net::{TcpListener, TcpStream},
    sync::{Arc, Mutex},
    time::Duration,
};

/// An HTTP request as received by the mock.
#[derive(Clone, Debug, Default)]
pub struct Request {
    /// Request method.
    pub method: String,
    /// Path and query, without decoding.
    pub target: String,
    /// Lowercase header names and their trimmed values.
    pub headers: Vec<(String, String)>,
    /// Unmodified body bytes, including multipart or non-UTF-8 content.
    pub body: Vec<u8>,
}
impl Request {
    /// Read the last occurrence of a header, case insensitively.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .rev()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }
}

/// A response whose explicit headers override the default type and length.
pub struct Response {
    /// HTTP status code.
    pub status: u16,
    /// Extra headers, including intentionally incorrect Content-Length values.
    pub headers: Vec<(&'static str, String)>,
    /// Unmodified response bytes.
    pub body: Vec<u8>,
}

/// Construct a JSON response from text.
pub fn reply(status: u16, body: &str) -> Response {
    Response {
        status,
        headers: Vec::new(),
        body: body.as_bytes().to_vec(),
    }
}

/// A mock's address and captured requests. Worker threads remain detached,
/// as in the original client fixtures, until the test process exits.
pub struct MockServer {
    url: String,
    calls: Arc<Mutex<Vec<Request>>>,
}
impl MockServer {
    /// Base HTTP URL on an ephemeral loopback port.
    pub fn url(&self) -> &str {
        &self.url
    }
    /// Snapshot the requests received so far, in handler-number order.
    pub fn calls(&self) -> Vec<Request> {
        self.calls.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

/// Start a concurrent mock. The handler receives the request and its zero-based number.
/// Returning `None` leaves that connection unanswered for timeout tests.
pub fn serve_http(
    handler: impl Fn(&Request, usize) -> Option<Response> + Send + Sync + 'static,
) -> io::Result<MockServer> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}", listener.local_addr()?);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let kept = Arc::clone(&calls);
    let handler = Arc::new(handler);
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(stream) = stream else { continue };
            let calls = Arc::clone(&kept);
            let handler = Arc::clone(&handler);
            std::thread::spawn(move || {
                if let Err(error) = respond(stream, &calls, &*handler) {
                    // Clients may deliberately time out or reject a malformed response.
                    if !matches!(
                        error.kind(),
                        io::ErrorKind::BrokenPipe
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::UnexpectedEof
                    ) {
                        eprintln!("HTTP mock: {error}");
                    }
                }
            });
        }
    });
    Ok(MockServer { url, calls })
}

fn respond(
    mut stream: TcpStream,
    calls: &Mutex<Vec<Request>>,
    handler: &impl Fn(&Request, usize) -> Option<Response>,
) -> io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut line = String::new();
    reader.read_line(&mut line)?;
    let mut parts = line.split(' ');
    let mut request = Request {
        method: parts.next().unwrap_or_default().into(),
        target: parts.next().unwrap_or_default().into(),
        ..Default::default()
    };
    loop {
        let mut line = String::new();
        reader.read_line(&mut line)?;
        if line.trim().is_empty() {
            break;
        }
        let (name, value) = line.split_once(':').unwrap_or_default();
        request
            .headers
            .push((name.to_ascii_lowercase(), value.trim().into()));
    }
    let len = request
        .header("content-length")
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(io::Error::other)?;
    request.body.resize(len, 0);
    reader.read_exact(&mut request.body)?;
    let n = {
        let mut log = calls.lock().unwrap_or_else(|e| e.into_inner());
        log.push(request.clone());
        log.len() - 1
    };
    let Some(answer) = handler(&request, n) else {
        std::thread::sleep(Duration::from_secs(3600));
        return Ok(());
    };
    let mut head = format!("HTTP/1.1 {} X\r\nConnection: close\r\n", answer.status);
    if !answer
        .headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-type"))
    {
        head.push_str("Content-Type: application/json\r\n");
    }
    for (key, value) in &answer.headers {
        head.push_str(&format!("{key}: {value}\r\n"));
    }
    if !answer
        .headers
        .iter()
        .any(|(k, _)| k.eq_ignore_ascii_case("content-length"))
    {
        head.push_str(&format!("Content-Length: {}\r\n", answer.body.len()));
    }
    head.push_str("\r\n");
    stream.write_all(head.as_bytes())?;
    stream.write_all(&answer.body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_bodies_and_explicit_response_headers_survive() {
        let server = serve_http(|request, n| {
            assert_eq!(n, 0);
            assert_eq!(request.body, [0, 255, b'x']);
            assert_eq!(request.header("Authorization"), Some("Bearer test"));
            Some(Response {
                status: 200,
                headers: vec![
                    ("Content-Length", "200".into()),
                    ("Content-Type", "application/octet-stream".into()),
                ],
                body: vec![255, 0, 128],
            })
        })
        .unwrap();
        let mut stream = TcpStream::connect(server.url().trim_start_matches("http://")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        stream.write_all(b"POST /upload?q=1 HTTP/1.1\r\nAuthorization: Bearer test\r\nContent-Length: 3\r\n\r\n\0\xffx").unwrap();
        let mut bytes = Vec::new();
        stream.read_to_end(&mut bytes).unwrap();
        let split = bytes.windows(4).position(|p| p == b"\r\n\r\n").unwrap() + 4;
        assert_eq!(&bytes[split..], &[255, 0, 128]);
        let headers = std::str::from_utf8(&bytes[..split]).unwrap();
        assert!(headers.contains("Content-Length: 200\r\n"));
        assert_eq!(headers.matches("Content-Type:").count(), 1);
        assert_eq!(server.calls()[0].target, "/upload?q=1");
        assert_eq!(server.calls()[0].body, [0, 255, b'x']);
    }
}
