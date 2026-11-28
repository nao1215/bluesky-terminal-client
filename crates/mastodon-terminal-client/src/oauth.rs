//! Logging in with OAuth, as Mastodon's own apps do. mstdn registers itself
//! on the server once (`POST /api/v1/apps`, kept in `apps/<host>.json`),
//! opens the server's authorization page in the browser, and the server
//! sends the browser back with a code: to a listener mstdn opens on
//! `http://127.0.0.1:<port>/callback`, or, with `--oob`, on a page that
//! shows the code to paste. The code is exchanged for the token at
//! `POST /oauth/token`, with a PKCE verifier (RFC 7636) so a code read on
//! its way is worth nothing to anyone else. mstdn never sees the password.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::time::{Duration, Instant};

use base64::Engine;
use sha2::Digest;

use crate::api::{Client, percent_encode};
use crate::config::{AccountStore, App};
use crate::error::{Error, Kind, Result};
use crate::i18n::t;

/// The name the server shows on its page that asks for the access.
pub(crate) const APP_NAME: &str = "mstdn";

/// The address the server shows beside the name.
pub(crate) const WEBSITE: &str = "https://github.com/nao1215/social-terminal-clients";

/// What mstdn asks to do: read what the account sees, and write (post,
/// upload the files of a post, and what later commands do). Mastodon's
/// `follow` and `push` are not asked for.
pub(crate) const SCOPES: &str = "read write";

/// The redirect that has the server show the code instead of sending the
/// browser anywhere.
pub(crate) const OOB: &str = "urn:ietf:wg:oauth:2.0:oob";

/// How long `mstdn login` waits for the access by default.
pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(300);

/// The address the listener on `port` takes the code at.
pub(crate) fn loopback_uri(port: u16) -> String {
    format!("http://127.0.0.1:{port}/callback")
}

/// The port of a loopback redirect `loopback_uri` made, if `uri` is one.
fn loopback_port(uri: &str) -> Option<u16> {
    uri.strip_prefix("http://127.0.0.1:")?
        .strip_suffix("/callback")?
        .parse()
        .ok()
}

/// A PKCE verifier and its S256 challenge.
pub(crate) struct Pkce {
    pub verifier: String,
    pub challenge: String,
}

/// A new PKCE pair: 32 random bytes as the verifier, its SHA-256 as the
/// challenge, both base64url without padding.
pub(crate) fn pkce() -> Result<Pkce> {
    let mut b = [0u8; 32];
    getrandom::fill(&mut b).map_err(|e| Error::io(format!("cannot log in: no randomness: {e}")))?;
    let verifier = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b);
    Ok(Pkce {
        challenge: challenge_of(&verifier),
        verifier,
    })
}

fn challenge_of(verifier: &str) -> String {
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(sha2::Sha256::digest(verifier))
}

/// The page on the server at `origin` that asks for the access.
pub(crate) fn authorize_url(
    origin: &str,
    client_id: &str,
    redirect: &str,
    state: &str,
    challenge: &str,
) -> String {
    format!(
        "{origin}/oauth/authorize?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        percent_encode(client_id),
        percent_encode(redirect),
        percent_encode(SCOPES),
        percent_encode(state),
        percent_encode(challenge),
    )
}

/// Where the code comes back: a listener on this machine, or the page
/// that shows it.
pub(crate) enum Redirect {
    Loopback(TcpListener),
    Oob,
}

impl Redirect {
    pub(crate) fn uri(&self) -> Result<String> {
        match self {
            Redirect::Loopback(l) => l
                .local_addr()
                .map(|a| loopback_uri(a.port()))
                .map_err(|e| Error::io(format!("cannot listen for the login: {e}"))),
            Redirect::Oob => Ok(OOB.to_string()),
        }
    }
}

/// The app to log in with on `client`'s server and where its code comes
/// back: the app saved for the server when it can take the code there (its
/// port free, for a loopback), else one registered now, and saved.
pub(crate) fn app_for(store: &AccountStore, client: &Client, oob: bool) -> Result<(App, Redirect)> {
    let host = client.host();
    let saved = store.app(host).filter(|a| a.service == client.origin());
    if let Some(app) = saved {
        if oob && app.redirect_uris.iter().any(|u| u == OOB) {
            return Ok((app, Redirect::Oob));
        }
        if !oob
            && let Some(port) = app.redirect_uris.iter().find_map(|u| loopback_port(u))
            && let Ok(l) = TcpListener::bind(("127.0.0.1", port))
        {
            return Ok((app, Redirect::Loopback(l)));
        }
    }
    let redirect = if oob {
        Redirect::Oob
    } else {
        Redirect::Loopback(TcpListener::bind(("127.0.0.1", 0)).map_err(|e| {
            Error::io(format!("cannot listen for the login: {e}"))
                .with_hint("pass --oob to paste the code instead")
        })?)
    };
    let mut uris = vec![redirect.uri()?];
    if !oob {
        uris.push(OOB.to_string());
    }
    let r = client.register_app(&uris, SCOPES)?;
    let app = App {
        service: client.origin().to_string(),
        client_id: r.client_id,
        client_secret: r.client_secret,
        redirect_uris: uris,
        scopes: SCOPES.to_string(),
    };
    store.save_app(&app)?;
    Ok((app, redirect))
}

/// Wait on `listener` for the browser to bring the code back with `state`,
/// for up to `timeout`. Anything else that knocks is turned away; the
/// browser is told whether to go back to the terminal.
pub(crate) fn wait_for_code(
    listener: &TcpListener,
    state: &str,
    host: &str,
    timeout: Duration,
) -> Result<String> {
    wait_for_code_until(listener, state, host, timeout, || false)
}

/// The TUI can cancel while the browser is open without saving a session.
pub(crate) fn wait_for_code_until(
    listener: &TcpListener,
    state: &str,
    host: &str,
    timeout: Duration,
    cancelled: impl Fn() -> bool,
) -> Result<String> {
    listener
        .set_nonblocking(true)
        .map_err(|e| Error::io(format!("cannot listen for the login: {e}")))?;
    let start = Instant::now();
    loop {
        if cancelled() {
            return Err(Error::usage("login cancelled"));
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(done) = answer(stream, state, host) {
                    return done;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                if start.elapsed() >= timeout {
                    return Err(Error::new(
                        Kind::Usage,
                        format!("the access was not given on {host} within {} s", timeout.as_secs()),
                    )
                    .with_hint("nothing was saved; run `mstdn login` and allow it on the page, or pass --oob to paste the code"));
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(Error::io(format!("cannot listen for the login: {e}"))),
        }
    }
}

/// Read one request from the browser and answer it; the end of the wait
/// when it brought the code or a refusal.
fn answer(stream: TcpStream, state: &str, host: &str) -> Option<Result<String>> {
    // The accepted stream inherits nothing, but on some systems it is
    // non-blocking like the listener; a slow browser gets a few seconds.
    let configured = stream.set_nonblocking(false).is_ok()
        && stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .is_ok();
    if !configured {
        return None;
    }
    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return None;
    }
    let target = line.split_whitespace().nth(1).unwrap_or("");
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let param = |name: &str| {
        query
            .split('&')
            .filter_map(|kv| kv.split_once('='))
            .find(|(k, _)| *k == name)
            .map(|(_, v)| decode_query(v))
    };
    let mut out = &stream;
    if path != "/callback" {
        reply(&mut out, 404, t("Nothing here."));
        return None;
    }
    if param("state").as_deref() != Some(state) {
        reply(
            &mut out,
            400,
            t("This is not the login mstdn is waiting for."),
        );
        return None;
    }
    if let Some(err) = param("error") {
        reply(
            &mut out,
            200,
            t("The access was not given. You can close this tab."),
        );
        let why = param("error_description")
            .map(|d| format!("{err}: {d}"))
            .unwrap_or(err);
        return Some(Err(Error::new(
            Kind::Usage,
            format!(
                "{host} did not give mstdn the access: {}",
                crate::text::one_line(&why)
            ),
        )
        .with_hint("nothing was saved; run `mstdn login` to ask again")));
    }
    match param("code").filter(|c| !c.is_empty()) {
        Some(code) => {
            reply(
                &mut out,
                200,
                t("mstdn is logged in. You can close this tab and go back to the terminal."),
            );
            Some(Ok(code))
        }
        None => {
            reply(&mut out, 400, t("The server sent no code."));
            None
        }
    }
}

/// Answer the browser with `status` and a page that says `text`.
fn reply(out: &mut dyn Write, status: u16, text: &str) {
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "Not Found",
    };
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>mstdn</title><p>{}</p>\n",
        text.replace('&', "&amp;").replace('<', "&lt;")
    );
    #[expect(
        clippy::let_underscore_must_use,
        reason = "the page is a courtesy to the browser; the login goes on whether it was shown or not"
    )]
    let _: std::io::Result<()> = write!(
        out,
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
}

/// A query value decoded: `+` as a space, `%XX` as its byte.
fn decode_query(v: &str) -> String {
    let b = v.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        let hex = |c: u8| (c as char).to_digit(16);
        if b[i] == b'%'
            && let (Some(h), Some(l)) = (
                b.get(i + 1).copied().and_then(hex),
                b.get(i + 2).copied().and_then(hex),
            )
        {
            out.push((h * 16 + l) as u8);
            i += 3;
            continue;
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The code pasted on `input`, as the page with `--oob` shows it.
pub(crate) fn read_code(input: &mut dyn BufRead) -> Result<String> {
    let mut line = String::new();
    input
        .read_line(&mut line)
        .map_err(|e| Error::io(format!("cannot read the code: {e}")))?;
    let code = line.trim();
    if code.is_empty() {
        return Err(Error::usage("no code was given").with_hint(
            "paste the code the page shows after you allow the access; nothing was saved",
        ));
    }
    Ok(code.to_string())
}

/// Trade `code` for the token at the server of `client`. A server that
/// does not know the app any more has it forgotten here, so the next login
/// registers again.
pub(crate) fn exchange(
    store: &AccountStore,
    client: &Client,
    app: &App,
    code: &str,
    redirect: &str,
    verifier: &str,
) -> Result<crate::api::types::Token> {
    let form = [
        ("grant_type", "authorization_code"),
        ("code", code),
        ("client_id", app.client_id.as_str()),
        ("client_secret", app.client_secret.as_str()),
        ("redirect_uri", redirect),
        ("code_verifier", verifier),
        ("scope", SCOPES),
    ];
    let host = client.host();
    match client.token(&form)? {
        Ok(t) => Ok(t),
        Err(r) if r.code() == "invalid_client" => {
            store.remove_app(host)?;
            Err(Error::new(
                Kind::Usage,
                format!(
                    "{host} no longer knows the app mstdn registered: {}",
                    crate::text::one_line(&r.said())
                ),
            )
            .with_hint("nothing was saved, and the app is forgotten; run `mstdn login` again"))
        }
        Err(r) if (400..500).contains(&r.status) => Err(Error::new(
            Kind::Usage,
            format!(
                "{host} did not take the code: {}",
                crate::text::one_line(&r.said())
            ),
        )
        .with_hint("nothing was saved; run `mstdn login` to ask again")),
        Err(r) => Err(Error::api(format!(
            "oauth/token failed: {}",
            crate::text::one_line(&r.said())
        ))),
    }
}

/// Verify the exchanged token and build a session. Saving is the caller's decision.
pub(crate) fn session(
    client: &Client,
    token: crate::api::types::Token,
) -> Result<crate::config::Session> {
    let me = Client::new(client.origin())
        .with_token(Some(token.access_token.clone()))
        .me()?;
    let scopes: Vec<String> = if token.scope.trim().is_empty() {
        SCOPES.split(' ').map(str::to_string).collect()
    } else {
        token.scope.split_whitespace().map(str::to_string).collect()
    };
    let saved = crate::config::Session {
        service: client.origin().to_string(),
        user_id: me.id,
        username: me.username,
        token: token.access_token,
        scopes,
    };
    Ok(saved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    // RFC 7636's own example: the challenge of its verifier.
    #[test]
    fn the_challenge_is_the_sha256_of_the_verifier() {
        assert_eq!(
            challenge_of("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        let p = pkce().unwrap();
        assert_eq!(p.verifier.len(), 43);
        assert_eq!(p.challenge, challenge_of(&p.verifier));
        assert_ne!(p.verifier, pkce().unwrap().verifier);
    }

    #[test]
    fn the_page_asks_for_reading_and_writing_with_pkce() {
        let url = authorize_url("https://m.test", "c id", &loopback_uri(8123), "s1", "ch");
        assert_eq!(
            url,
            "https://m.test/oauth/authorize?response_type=code&client_id=c%20id&redirect_uri=http%3A%2F%2F127.0.0.1%3A8123%2Fcallback&scope=read%20write&state=s1&code_challenge=ch&code_challenge_method=S256"
        );
        assert_eq!(loopback_port(&loopback_uri(8123)), Some(8123));
        assert_eq!(loopback_port(OOB), None);
        assert_eq!(loopback_port("http://127.0.0.1:x/callback"), None);
    }

    /// What `GET target` on the listener at `port` answers.
    fn knock(port: u16, target: &str) -> String {
        let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
        write!(s, "GET {target} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n").unwrap();
        let mut got = String::new();
        s.read_to_string(&mut got).unwrap();
        got
    }

    // A knock without the state, or at another path, is turned away and the
    // wait goes on; the one with the state brings the code.
    #[test]
    fn the_listener_takes_the_code_that_comes_with_the_state() {
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let knocks = std::thread::spawn(move || {
            let a = knock(port, "/favicon.ico");
            let b = knock(port, "/callback?code=x&state=wrong");
            let c = knock(port, "/callback?code=c%2Bd+e&state=s%201");
            (a, b, c)
        });
        let code = wait_for_code(&l, "s 1", "m.test", Duration::from_secs(10)).unwrap();
        assert_eq!(code, "c+d e");
        let (a, b, c) = knocks.join().unwrap();
        assert!(a.starts_with("HTTP/1.1 404"), "{a}");
        assert!(b.starts_with("HTTP/1.1 400"), "{b}");
        assert!(
            c.starts_with("HTTP/1.1 200") && c.contains("close this tab"),
            "{c}"
        );
    }

    #[test]
    fn a_denial_or_no_answer_in_time_is_a_usage_error() {
        let l = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = l.local_addr().unwrap().port();
        let k = std::thread::spawn(move || {
            knock(
                port,
                "/callback?error=access_denied&error_description=The+user+said+no&state=s",
            )
        });
        let e = wait_for_code(&l, "s", "m.test", Duration::from_secs(10)).unwrap_err();
        assert_eq!(e.kind(), Kind::Usage);
        assert_eq!(
            e.message(),
            "m.test did not give mstdn the access: access_denied: The user said no"
        );
        k.join().unwrap();
        let e = wait_for_code(&l, "s", "m.test", Duration::from_millis(100)).unwrap_err();
        assert!(
            e.message().contains("was not given on m.test within 0 s"),
            "{e:?}"
        );
        assert!(e.hint().unwrap().contains("nothing was saved"));
    }

    #[test]
    fn a_pasted_code_is_trimmed_and_an_empty_one_refused() {
        assert_eq!(read_code(&mut " abc \n".as_bytes()).unwrap(), "abc");
        assert_eq!(
            read_code(&mut "\n".as_bytes()).unwrap_err().kind(),
            Kind::Usage
        );
        assert_eq!(
            read_code(&mut "".as_bytes()).unwrap_err().kind(),
            Kind::Usage
        );
    }
}
