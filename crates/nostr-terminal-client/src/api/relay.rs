//! Bounded synchronous WebSocket sessions, one per configured relay and operation.

use super::{
    event::{Event, Unsigned},
    nip11::{self, Limits},
};
use crate::error::{Error, Result};
use k256::schnorr::SigningKey;
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

/// Signs relay authentication with the same account signer as application writes.
pub(crate) trait AuthSigner: Sync {
    fn sign_auth(&self, event: Unsigned) -> Result<Event>;
}
impl AuthSigner for SigningKey {
    fn sign_auth(&self, event: Unsigned) -> Result<Event> {
        Event::sign(
            self,
            event.created_at,
            event.kind,
            event.tags,
            event.content,
        )
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub(crate) struct Filter {
    pub(crate) kinds: Vec<u16>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub(crate) search: Option<String>,
    #[serde(rename = "#t", skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) hashtags: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) ids: Vec<String>,
    #[serde(rename = "#e", skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) references: Vec<String>,
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
            && (self.hashtags.is_empty()
                || event
                    .tag_values("t")
                    .any(|tag| self.hashtags.iter().any(|wanted| wanted == tag)))
            && (self.ids.is_empty() || self.ids.contains(&event.id))
            && (self.references.is_empty()
                || event
                    .tag_values("e")
                    .any(|id| self.references.iter().any(|r| r == id)))
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
    Auth(String),
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
            "AUTH" if fields.len() == 2 => Ok(Self::Auth(string(1)?)),
            "NOTICE" if fields.len() == 2 => Ok(Self::Notice(string(1)?)),
            "CLOSED"
                if (3..=4).contains(&fields.len())
                    && fields.get(3).is_none_or(Value::is_string) =>
            {
                Ok(Self::Closed(string(1)?, string(2)?))
            }
            "EVENT" | "EOSE" | "OK" | "NOTICE" | "CLOSED" | "AUTH" => Err(invalid()),
            _ => Ok(Self::Unknown),
        }
    }
}

#[derive(Debug)]
pub(crate) struct DeadlineStream {
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

pub(crate) struct Session<S = MaybeTlsStream<DeadlineStream>> {
    socket: WebSocket<S>,
    deadline: Instant,
    notices: Vec<String>,
    limits: Limits,
    incomplete: bool,
}

impl Session {
    pub(crate) fn open_configured(address: &str, timeout: Duration) -> Result<Self> {
        let deadline = Instant::now() + timeout;
        let limits = nip11::limits(address, timeout / 4);
        let mut session = Self::open(address, remaining(deadline).map_err(Error::network)?)?;
        session.limits = limits;
        Ok(session)
    }

    pub(crate) fn open(address: &str, timeout: Duration) -> Result<Self> {
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
            limits: Limits::default(),
            incomplete: false,
        })
    }
}

impl<S: Read + Write> Session<S> {
    pub(crate) fn send(&mut self, message: Value) -> Result<()> {
        remaining(self.deadline).map_err(Error::network)?;
        let text = message.to_string();
        if self
            .limits
            .max_message_length
            .is_some_and(|max| text.len() > max)
        {
            return Err(Error::network(
                "relay limitation: max_message_length exceeded",
            ));
        }
        self.socket
            .send(Message::Text(text.into()))
            .map_err(Error::network)
    }

    pub(crate) fn receive(&mut self) -> Result<RelayMessage> {
        loop {
            remaining(self.deadline).map_err(|_timeout| Error::network("relay timeout"))?;
            let message = self.socket.read().map_err(|error| match &error {
                tungstenite::Error::Io(e)
                    if matches!(
                        e.kind(),
                        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                    ) =>
                {
                    Error::network("relay timeout")
                }
                _ => Error::network(error),
            })?;
            match message {
                Message::Text(text) => match RelayMessage::parse(&text)? {
                    RelayMessage::Notice(text) => {
                        if text.starts_with("unsupported:") {
                            return Err(Error::unsupported(text));
                        }
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
        self.subscribe(subscription, filter)?;
        let mut events = HashMap::new();
        loop {
            let message = match self.receive() {
                Ok(message) => message,
                Err(e) if e.error == "relay timeout" => {
                    self.incomplete = true;
                    let reason = "EOSE unavailable before timeout; results may be incomplete";
                    if events.is_empty() {
                        return Err(Error::network(reason));
                    }
                    self.notices.push(reason.into());
                    return Ok(events.into_values().collect());
                }
                Err(e) => return Err(e),
            };
            match message {
                RelayMessage::Auth(_) => {
                    return Err(Error::unsupported(
                        "auth-required: relay requires auth; reads do not open a signing key",
                    ));
                }
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
                    return Err(
                        if reason.starts_with("unsupported:")
                            || reason.starts_with("auth-required:")
                        {
                            Error::unsupported(format!("subscription closed: {reason}"))
                        } else {
                            Error::network(format!("subscription closed: {reason}"))
                        },
                    );
                }
                _ => (),
            }
        }
    }

    fn publish(
        &mut self,
        event: &Event,
        address: &str,
        signer: Option<&dyn AuthSigner>,
    ) -> Result<()> {
        if self.limits.payment_required {
            return Err(Error::unsupported("unsupported: payment-required relay"));
        }
        if self.limits.auth_required && signer.is_none() {
            return Err(Error::unsupported(
                "auth-required: relay requires auth; account signer unavailable",
            ));
        }
        self.send(json!(["EVENT", event]))?;
        let mut auth_id = None;
        let mut authenticated = false;
        let mut rejection = None;
        let mut retried = false;
        loop {
            let message = self.receive().map_err(|mut e| {
                if let Some(reason) = &rejection {
                    e.error = format!(
                        "publish rejected: {reason}; authentication incomplete: {}",
                        e.error
                    );
                }
                e
            })?;
            match message {
                RelayMessage::Auth(challenge) => {
                    if auth_id.is_some() {
                        return Err(Error::network("auth-required: repeated AUTH challenge"));
                    }
                    let signer = signer.ok_or_else(|| {
                        Error::unsupported(
                            "auth-required: relay requires auth; account signer unavailable",
                        )
                    })?;
                    let now = u64::try_from(jiff::Timestamp::now().as_second())
                        .map_err(Error::network)?;
                    let auth = signer.sign_auth(Unsigned {
                        created_at: now,
                        kind: 22242,
                        tags: vec![
                            vec!["relay".into(), address.into()],
                            vec!["challenge".into(), challenge],
                        ],
                        content: String::new(),
                    })?;
                    auth.verify()?;
                    self.send(json!(["AUTH", auth]))?;
                    auth_id = Some(auth.id);
                }
                RelayMessage::Ok(id, accepted, reason) if auth_id.as_ref() == Some(&id) => {
                    if !accepted {
                        let message = format!("authentication rejected: {reason}");
                        return Err(if reason.starts_with("unsupported:") {
                            Error::unsupported(message)
                        } else {
                            Error::network(message)
                        });
                    }
                    authenticated = true;
                }
                RelayMessage::Ok(id, accepted, reason) if id == event.id => {
                    if accepted || reason.starts_with("duplicate:") {
                        return Ok(());
                    }
                    if reason.starts_with("unsupported:") {
                        return Err(Error::unsupported(format!("publish rejected: {reason}")));
                    }
                    if !reason.starts_with("auth-required:") || retried {
                        return Err(Error::network(format!("publish rejected: {reason}")));
                    }
                    if signer.is_none() {
                        return Err(Error::unsupported(format!("publish rejected: {reason}")));
                    }
                    rejection = Some(reason);
                }
                _ => (),
            }
            if authenticated && rejection.is_some() && !retried {
                self.send(json!(["EVENT", event]))?;
                retried = true;
            }
        }
    }

    fn check_read(&self, filter: &Filter) -> Result<()> {
        if self.limits.payment_required {
            return Err(Error::unsupported("unsupported: payment-required relay"));
        }
        if self.limits.auth_required {
            return Err(Error::unsupported(
                "auth-required: relay requires auth; reads do not open a signing key",
            ));
        }
        if self.limits.max_subscriptions == Some(0) {
            return Err(Error::unsupported(
                "unsupported: subscriptions (max_subscriptions=0)",
            ));
        }
        if self.limits.max_limit == Some(0) {
            return Err(Error::unsupported("unsupported: reads (max_limit=0)"));
        }
        if filter.search.is_some() && !self.limits.supported_nips.contains(&50) {
            return Err(Error::unsupported(
                "unsupported: NIP-50 search is not advertised by this relay",
            ));
        }
        Ok(())
    }

    pub(crate) fn subscribe(&mut self, subscription: &str, filter: &Filter) -> Result<()> {
        self.check_read(filter)?;
        let mut filter = filter.clone();
        if let Some(max) = self.limits.max_limit {
            filter.limit = filter.limit.min(max);
        }
        self.send(json!(["REQ", subscription, filter]))
    }

    fn read(&mut self, filter: &Filter, count: usize) -> Result<Vec<Event>> {
        self.check_read(filter)?;
        let mut query = filter.clone();
        query.limit = count.min(100);
        let mut events = HashMap::new();
        let mut page_number = 0;
        let mut short_page_oldest = None;
        while events.len() < count {
            if let Some(max) = self.limits.max_limit
                && query.limit > max
            {
                if !self
                    .notices
                    .iter()
                    .any(|n| n.starts_with("relay limitation: max_limit"))
                {
                    self.notices.push(format!(
                        "relay limitation: max_limit={max}; using smaller pages"
                    ));
                }
                query.limit = max;
            }
            let page = match self.page(&query, &format!("nstr-{page_number}")) {
                Ok(page) => page,
                Err(e) if self.incomplete && !events.is_empty() => {
                    self.notices.push(e.error);
                    break;
                }
                Err(e) => return Err(e),
            };
            page_number += 1;
            let Some(oldest) = page.iter().map(|e| e.created_at).min() else {
                break;
            };
            if self.limits.max_limit.is_none()
                && short_page_oldest.is_some_and(|previous| oldest < previous)
                && !self.notices.iter().any(|n| n.starts_with("relay capped"))
            {
                self.notices.push(
                    "relay capped results below the requested limit; using smaller pages".into(),
                );
            }
            short_page_oldest = (page.len() < query.limit).then_some(oldest);
            let page_len = page.len();
            let before = events.len();
            for event in page {
                events.insert(event.id.clone(), event);
            }
            if self.incomplete || events.len() >= count {
                break;
            }
            if events.len() == before {
                if page_len >= query.limit
                    && self.limits.max_limit.is_some_and(|max| query.limit >= max)
                {
                    if !self
                        .notices
                        .iter()
                        .any(|n| n.contains("timestamp boundary"))
                    {
                        self.notices.push("relay limitation: timestamp boundary cannot be exhausted at max_limit; results may be incomplete".into());
                    }
                    let Some(earlier) = oldest.checked_sub(1) else {
                        break;
                    };
                    query.until = Some(earlier);
                } else if page_len >= query.limit {
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
    pub(crate) answered: usize,
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
        operation: impl Fn(&mut Session, &str) -> Result<T> + Sync,
    ) -> Result<Outcome<Vec<T>>> {
        let results = thread::scope(|scope| {
            let handles: Vec<_> = self
                .urls
                .iter()
                .map(|url| {
                    let operation = &operation;
                    scope.spawn(move || {
                        let mut session = Session::open_configured(url, self.timeout)?;
                        let result = operation(&mut session, url);
                        let notices = session.notices.join("; ");
                        result
                            .map(|value| (value, notices.clone()))
                            .map_err(|mut e| {
                                e.error = format!(
                                    "{}{}",
                                    e.error,
                                    if notices.is_empty() {
                                        String::new()
                                    } else {
                                        format!("; NOTICE: {notices}")
                                    }
                                );
                                e
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
        let mut only_unsupported = true;
        for (url, result) in self.urls.iter().zip(results) {
            match result {
                Ok((value, notices)) => {
                    values.push(value);
                    if !notices.is_empty() {
                        warnings.push(format!("{url}: {notices}"));
                    }
                }
                Err(e) => {
                    only_unsupported &= e.status == 5;
                    warnings.push(format!("{url}: {}", e.error));
                }
            }
        }
        if values.is_empty() {
            let message = format!("no relay completed the operation: {}", warnings.join("; "));
            return Err(if only_unsupported {
                Error::unsupported(message)
            } else {
                Error::network(message)
            });
        }
        Ok(Outcome {
            answered: values.len(),
            value: values,
            warnings,
        })
    }

    pub(crate) fn query(&self, filter: Filter, count: usize) -> Result<Outcome<Vec<Event>>> {
        let results = self.each(|s, _url| s.read(&filter, count))?;
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
            answered: results.answered,
            value: events,
            warnings: results.warnings,
        })
    }

    pub(crate) fn publish(
        &self,
        event: &Event,
        signer: Option<&dyn AuthSigner>,
    ) -> Result<Outcome<usize>> {
        event.verify()?;
        let results = self.each(|s, url| s.publish(event, url, signer))?;
        Ok(Outcome {
            answered: results.answered,
            value: results.value.len(),
            warnings: results.warnings,
        })
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
pub(crate) mod tests;
