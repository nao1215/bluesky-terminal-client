//! Bounded synchronous WebSocket sessions, one per configured relay and operation.

use super::event::Event;
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    io::{self, Read, Write},
    net::{TcpStream, ToSocketAddrs},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use tungstenite::{Message, WebSocket, protocol::WebSocketConfig, stream::MaybeTlsStream};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Filter {
    pub(crate) kinds: Vec<u16>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) authors: Vec<String>,
    #[serde(rename = "#p", skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) mentions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) until: Option<u64>,
    pub(crate) limit: usize,
}

impl Filter {
    pub(crate) fn matches(&self, event: &Event) -> bool {
        self.kinds.contains(&event.kind)
            && (self.authors.is_empty() || self.authors.contains(&event.pubkey))
            && (self.mentions.is_empty()
                || event
                    .tag_values("p")
                    .any(|p| self.mentions.iter().any(|m| m == p)))
            && self.until.is_none_or(|t| event.created_at <= t)
    }
}

#[derive(Debug, PartialEq)]
pub(crate) enum RelayMessage {
    Event(String, Event),
    Eose(String),
    Ok(String, bool, String),
    Notice(String),
    Closed(String, String),
    Unknown,
}

impl RelayMessage {
    pub(crate) fn parse(text: &str) -> Result<Self> {
        let value: Value = serde_json::from_str(text)
            .map_err(|_invalid| Error::network("relay sent malformed JSON"))?;
        let fields = value
            .as_array()
            .ok_or_else(|| Error::network("relay message must be an array"))?;
        let invalid = || Error::network("relay message has invalid fields");
        let string = |i: usize| {
            fields
                .get(i)
                .and_then(Value::as_str)
                .map(str::to_owned)
                .ok_or_else(invalid)
        };
        let name = string(0)?;
        match name.as_str() {
            "EVENT" if fields.len() == 3 => Ok(Self::Event(
                string(1)?,
                serde_json::from_value(fields[2].clone()).map_err(|_invalid| invalid())?,
            )),
            "EOSE" if fields.len() == 2 => Ok(Self::Eose(string(1)?)),
            "OK" if (4..=5).contains(&fields.len())
                && fields.get(4).is_none_or(Value::is_string) =>
            {
                Ok(Self::Ok(
                    string(1)?,
                    fields[2].as_bool().ok_or_else(invalid)?,
                    string(3)?,
                ))
            }
            "NOTICE" if fields.len() == 2 => Ok(Self::Notice(string(1)?)),
            "CLOSED"
                if (3..=4).contains(&fields.len())
                    && fields.get(3).is_none_or(Value::is_string) =>
            {
                Ok(Self::Closed(string(1)?, string(2)?))
            }
            "EVENT" | "EOSE" | "OK" | "NOTICE" | "CLOSED" => Err(invalid()),
            _ => Ok(Self::Unknown),
        }
    }
}

#[derive(Debug)]
struct DeadlineStream {
    stream: TcpStream,
    deadline: Instant,
}

fn remaining(deadline: Instant) -> io::Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| io::Error::new(io::ErrorKind::TimedOut, "relay deadline exceeded"))
}

impl Read for DeadlineStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.stream
            .set_read_timeout(Some(remaining(self.deadline)?))?;
        self.stream.read(buf)
    }
}

impl Write for DeadlineStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stream
            .set_write_timeout(Some(remaining(self.deadline)?))?;
        self.stream.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.stream.flush()
    }
}

struct Session<S = MaybeTlsStream<DeadlineStream>> {
    socket: WebSocket<S>,
    deadline: Instant,
    notices: Vec<String>,
}

impl Session {
    fn open(address: &str, timeout: Duration) -> Result<Self> {
        let deadline = Instant::now() + timeout;
        let url = url::Url::parse(address).map_err(Error::network)?;
        let host = url
            .host_str()
            .ok_or_else(|| Error::network("relay URL has no host"))?
            .trim_matches(['[', ']'])
            .to_owned();
        let port = url
            .port_or_known_default()
            .ok_or_else(|| Error::network("relay URL has no port"))?;
        // The resolver can block outside socket timeouts; the caller stops waiting at the same deadline.
        let (send, recv) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map(|a| a.collect::<Vec<_>>());
            if send.send(result).is_err() { /* The operation has already timed out. */ }
        });
        let addresses = recv
            .recv_timeout(remaining(deadline).map_err(Error::network)?)
            .map_err(Error::network)?
            .map_err(Error::network)?;
        let mut last = Error::network("relay host resolved to no addresses");
        let mut connected = None;
        for address in addresses {
            match TcpStream::connect_timeout(&address, remaining(deadline).map_err(Error::network)?)
            {
                Ok(stream) => {
                    connected = Some(stream);
                    break;
                }
                Err(e) => last = Error::network(e),
            }
        }
        let stream = DeadlineStream {
            stream: connected.ok_or(last)?,
            deadline,
        };
        let config = WebSocketConfig::default()
            .max_message_size(Some(2 * 1024 * 1024))
            .max_frame_size(Some(2 * 1024 * 1024));
        let (socket, _) = tungstenite::client_tls_with_config(address, stream, Some(config), None)
            .map_err(Error::network)?;
        Ok(Self {
            socket,
            deadline,
            notices: Vec::new(),
        })
    }
}

impl<S: Read + Write> Session<S> {
    fn send(&mut self, message: Value) -> Result<()> {
        remaining(self.deadline).map_err(Error::network)?;
        self.socket
            .send(Message::Text(message.to_string().into()))
            .map_err(Error::network)
    }

    fn receive(&mut self) -> Result<RelayMessage> {
        loop {
            remaining(self.deadline).map_err(Error::network)?;
            match self.socket.read().map_err(Error::network)? {
                Message::Text(text) => match RelayMessage::parse(&text)? {
                    RelayMessage::Notice(text) => {
                        if self.notices.len() < 16 {
                            self.notices.push(text);
                        }
                    }
                    message => return Ok(message),
                },
                Message::Ping(_) => self.socket.flush().map_err(Error::network)?,
                Message::Close(_) => {
                    return Err(Error::network("relay closed before completing the request"));
                }
                _ => (),
            }
        }
    }

    fn page(&mut self, filter: &Filter, subscription: &str) -> Result<Vec<Event>> {
        self.send(json!(["REQ", subscription, filter]))?;
        let mut events = HashMap::new();
        loop {
            match self.receive()? {
                RelayMessage::Event(sub, event) if sub == subscription => {
                    event.verify()?;
                    if !filter.matches(&event) {
                        return Err(Error::network(
                            "relay sent an event outside the requested filter",
                        ));
                    }
                    events.insert(event.id.clone(), event);
                    if events.len() > filter.limit {
                        return Err(Error::network("relay exceeded the requested event limit"));
                    }
                }
                RelayMessage::Eose(sub) if sub == subscription => {
                    self.send(json!(["CLOSE", subscription]))?;
                    return Ok(events.into_values().collect());
                }
                RelayMessage::Closed(sub, reason) if sub == subscription => {
                    return Err(Error::network(format!("subscription closed: {reason}")));
                }
                _ => (),
            }
        }
    }

    fn publish(&mut self, event: &Event) -> Result<()> {
        self.send(json!(["EVENT", event]))?;
        loop {
            if let RelayMessage::Ok(id, accepted, reason) = self.receive()?
                && id == event.id
            {
                return if accepted {
                    Ok(())
                } else {
                    Err(Error::network(format!("publish rejected: {reason}")))
                };
            }
        }
    }

    fn read(&mut self, filter: &Filter, count: usize) -> Result<Vec<Event>> {
        let mut query = filter.clone();
        query.limit = count.min(100);
        let mut events = HashMap::new();
        let mut page_number = 0;
        while events.len() < count {
            let page = self.page(&query, &format!("nstr-{page_number}"))?;
            page_number += 1;
            let Some(oldest) = page.iter().map(|e| e.created_at).min() else {
                break;
            };
            let page_len = page.len();
            let before = events.len();
            for event in page {
                events.insert(event.id.clone(), event);
            }
            if events.len() >= count {
                break;
            }
            if events.len() == before {
                if page_len >= query.limit {
                    query.limit = query.limit.saturating_mul(2);
                    if query.limit > 10000 {
                        return Err(Error::network(
                            "too many events at one timestamp to page without omissions",
                        ));
                    }
                } else {
                    let Some(earlier) = oldest.checked_sub(1) else {
                        break;
                    };
                    query.until = Some(earlier);
                    query.limit = (count - events.len()).min(100);
                }
            } else {
                query.until = Some(oldest);
                let boundary = events.values().filter(|e| e.created_at == oldest).count();
                query.limit = (count - events.len()).min(100).saturating_add(boundary);
                if query.limit > 10000 {
                    return Err(Error::network(
                        "too many events at one timestamp to page without omissions",
                    ));
                }
            }
        }
        Ok(events.into_values().collect())
    }
}

pub(crate) struct Outcome<T> {
    pub(crate) value: T,
    pub(crate) warnings: Vec<String>,
}

pub(crate) struct Relays<'a> {
    pub(crate) urls: &'a [String],
    pub(crate) timeout: Duration,
}

impl Relays<'_> {
    fn each<T: Send>(
        &self,
        operation: impl Fn(&mut Session) -> Result<T> + Sync,
    ) -> Result<Outcome<Vec<T>>> {
        let results = thread::scope(|scope| {
            let handles: Vec<_> = self
                .urls
                .iter()
                .map(|url| {
                    let operation = &operation;
                    scope.spawn(move || {
                        let mut session = Session::open(url, self.timeout)?;
                        let result = operation(&mut session);
                        let notices = session.notices.join("; ");
                        result.map(|value| (value, notices.clone())).map_err(|e| {
                            Error::network(format!(
                                "{}{}",
                                e.error,
                                if notices.is_empty() {
                                    String::new()
                                } else {
                                    format!("; NOTICE: {notices}")
                                }
                            ))
                        })
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|h| {
                    h.join()
                        .unwrap_or_else(|_| Err(Error::network("relay worker failed")))
                })
                .collect::<Vec<_>>()
        });
        let mut values = Vec::new();
        let mut warnings = Vec::new();
        for (url, result) in self.urls.iter().zip(results) {
            match result {
                Ok((value, notices)) => {
                    values.push(value);
                    if !notices.is_empty() {
                        warnings.push(format!("{url}: {notices}"));
                    }
                }
                Err(e) => warnings.push(format!("{url}: {}", e.error)),
            }
        }
        if values.is_empty() {
            return Err(Error::network(format!(
                "no relay completed the operation: {}",
                warnings.join("; ")
            )));
        }
        Ok(Outcome {
            value: values,
            warnings,
        })
    }

    pub(crate) fn query(&self, filter: Filter, count: usize) -> Result<Outcome<Vec<Event>>> {
        let results = self.each(|s| s.read(&filter, count))?;
        let mut seen = HashSet::new();
        let mut events: Vec<_> = results
            .value
            .into_iter()
            .flatten()
            .filter(|e| seen.insert(e.id.clone()))
            .collect();
        events.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        events.truncate(count);
        Ok(Outcome {
            value: events,
            warnings: results.warnings,
        })
    }

    pub(crate) fn publish(&self, event: &Event) -> Result<Outcome<usize>> {
        event.verify()?;
        let results = self.each(|s| s.publish(event))?;
        Ok(Outcome {
            value: results.value.len(),
            warnings: results.warnings,
        })
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
