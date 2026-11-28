//! Signer-initiated NIP-46 connections with one disposable client key per post.

use super::{
    event::{Event, bytes},
    nip44,
    relay::{RelayMessage, Session},
};
use crate::{
    config::relay_urls,
    error::{Error, Result},
};
use k256::{
    elliptic_curve::{
        Generate,
        common::{getrandom::SysRng, rand_core::TryRng},
    },
    schnorr::{SigningKey, VerifyingKey},
};
use serde::Deserialize;
use serde_json::json;
use std::{
    io::{Read, Write},
    sync::mpsc,
    thread,
    time::{Duration, Instant},
};
use zeroize::Zeroizing;

pub(crate) struct Bunker {
    public: String,
    relays: Vec<String>,
    secret: Option<Zeroizing<String>>,
}

impl Bunker {
    pub(crate) fn parse(uri: &str) -> Result<Self> {
        let invalid = || {
            Error::file(
                "bunker must be a bunker:// hex signer public key with relay parameters and an optional secret",
            )
        };
        let url = url::Url::parse(uri).map_err(|_invalid| invalid())?;
        if url.scheme() != "bunker"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.port().is_some()
            || !url.path().is_empty()
            || url.fragment().is_some()
            || uri.chars().any(char::is_whitespace)
        {
            return Err(invalid());
        }
        let public = url.host_str().ok_or_else(invalid)?.to_owned();
        VerifyingKey::from_slice(&bytes::<32>(&public).map_err(|_invalid| invalid())?)
            .map_err(|_invalid| invalid())?;
        let mut relays = Vec::new();
        let mut secret = None;
        for (key, value) in url.query_pairs() {
            match key.as_ref() {
                "relay" => relays.push(value.into_owned()),
                "secret" if secret.is_none() && !value.is_empty() => {
                    secret = Some(Zeroizing::new(value.into_owned()))
                }
                _ => return Err(invalid()),
            }
        }
        Ok(Self {
            public,
            relays: relay_urls(relays).map_err(|_invalid| invalid())?,
            secret,
        })
    }

    pub(crate) fn sign_note(
        &self,
        public: &str,
        time: u64,
        text: &str,
        timeout: Duration,
    ) -> Result<Event> {
        let key = SigningKey::generate();
        let client_public = hex::encode(key.verifying_key().to_bytes());
        let conversation = nip44::conversation_key(&key, &bytes(&self.public)?)?;
        let deadline = Instant::now() + timeout;
        thread::scope(|scope| {
            let (responses, incoming) = mpsc::sync_channel(32);
            let mut outgoing = Vec::new();
            for relay in &self.relays {
                let (send, requests) = mpsc::channel::<(String, Event)>();
                outgoing.push(send);
                let responses = responses.clone();
                let conversation = &conversation;
                let client_public = &client_public;
                scope.spawn(move || {
                    // Relay diagnostics can echo the connect secret; expose only our own errors.
                    let result = (|| {
                        let mut session = Session::open(relay, remaining(deadline)?)?;
                        Connection {
                            signer: &self.public,
                            client: client_public,
                            conversation,
                            deadline,
                        }.serve(&mut session, requests, &responses)
                    })();
                    if let Err(_failure) = result { /* Dropping the response sender reports an unavailable relay. */ }
                });
            }
            drop(responses);
            let rpc = |method: &str, params: Vec<String>| -> Result<String> {
                let id = hex::encode(random_bytes()?);
                let plaintext = Zeroizing::new(
                    json!({"id": id, "method": method, "params": params}).to_string(),
                );
                let content = nip44::encrypt(&conversation, &random_bytes()?, &plaintext)?;
                let now =
                    u64::try_from(jiff::Timestamp::now().as_second()).map_err(Error::network)?;
                let event = Event::sign(
                    &key,
                    now,
                    24133,
                    vec![vec!["p".into(), self.public.clone()]],
                    content,
                )?;
                for send in &outgoing {
                    if send.send((id.clone(), event.clone())).is_err() { /* Another configured relay may still answer. */
                    }
                }
                let mut auth_shown = false;
                loop {
                    let (reply_id, reply) =
                        incoming
                            .recv_timeout(remaining(deadline)?)
                            .map_err(|_unavailable| {
                                Error::network("remote signer unavailable or timed out")
                            })?;
                    if reply_id != id {
                        continue;
                    }
                    match reply {
                        Reply::Auth(url) => {
                            if !auth_shown {
                                crate::diagnostics(&format!(
                                    "Remote signer authorization required. Open this URL yourself: {url}"
                                ));
                                auth_shown = true;
                            }
                        }
                        Reply::Value(value) => return Ok(value),
                        Reply::Rejected => {
                            return Err(Error::network(format!("remote signer rejected {method}")));
                        }
                    }
                }
            };
            let result = (|| {
                let ack = rpc(
                    "connect",
                    vec![
                        self.public.clone(),
                        self.secret.as_deref().map_or("", String::as_str).into(),
                        "sign_event:1".into(),
                        json!({"name":"nstr"}).to_string(),
                    ],
                )?;
                if ack != "ack" {
                    return Err(Error::network("remote signer did not acknowledge connect"));
                }
                if rpc("get_public_key", vec![])? != public {
                    return Err(Error::network(
                        "remote signer public key does not match publicKey",
                    ));
                }
                let unsigned = json!({"kind":1, "content":text, "tags":[], "created_at":time});
                let signed = rpc("sign_event", vec![unsigned.to_string()])?;
                validate_note(&signed, public, time, text)
            })();
            drop(outgoing);
            drop(incoming);
            result
        })
    }
}

fn validate_note(signed: &str, public: &str, time: u64, text: &str) -> Result<Event> {
    let event: Event = serde_json::from_str(signed)
        .map_err(|_invalid| Error::network("remote signer returned an invalid event"))?;
    event.verify()?;
    if event.pubkey != public
        || event.created_at != time
        || event.kind != 1
        || !event.tags.is_empty()
        || event.content != text
    {
        return Err(Error::network("remote signer changed the requested event"));
    }
    Ok(event)
}

/// One relay subscription stays open across all three RPCs, including EOSE and auth challenges.
struct Connection<'a> {
    signer: &'a str,
    client: &'a str,
    conversation: &'a [u8; 32],
    deadline: Instant,
}

impl Connection<'_> {
    fn serve<S: Read + Write>(
        &self,
        session: &mut Session<S>,
        requests: mpsc::Receiver<(String, Event)>,
        responses: &mpsc::SyncSender<(String, Reply)>,
    ) -> Result<()> {
        session.send(json!(["REQ", "nstr-bunker", {
            "kinds": [24133], "authors": [self.signer], "#p": [self.client]
        }]))?;
        while let Ok((id, request)) = requests.recv_timeout(remaining(self.deadline)?) {
            session.send(json!(["EVENT", request]))?;
            loop {
                match session.receive()? {
                    RelayMessage::Event(sub, event) if sub == "nstr-bunker" => {
                        if let Some(reply) =
                            response(&event, self.signer, self.client, self.conversation, &id)?
                        {
                            let done = !matches!(reply, Reply::Auth(_));
                            if responses.send((id.clone(), reply)).is_err() {
                                return Ok(());
                            }
                            if done {
                                break;
                            }
                        }
                    }
                    RelayMessage::Ok(event_id, false, _) if event_id == request.id => {
                        return Err(Error::network("signer relay rejected request"));
                    }
                    RelayMessage::Closed(sub, _) if sub == "nstr-bunker" => {
                        return Err(Error::network("signer subscription closed"));
                    }
                    _ => (),
                }
            }
        }
        // Drop also closes the transport if the deadline prevents CLOSE.
        if session.send(json!(["CLOSE", "nstr-bunker"])).is_err() { /* Best effort at session end. */
        }
        Ok(())
    }
}

fn remaining(deadline: Instant) -> Result<Duration> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|d| !d.is_zero())
        .ok_or_else(|| Error::network("remote signer timed out"))
}

fn random_bytes() -> Result<[u8; 32]> {
    let mut bytes = [0; 32];
    SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_failed| Error::file("cannot obtain randomness for remote signing"))?;
    Ok(bytes)
}

#[derive(Debug, PartialEq)]
enum Reply {
    Value(String),
    Auth(String),
    Rejected,
}

#[derive(Deserialize)]
struct Response {
    id: String,
    result: Option<String>,
    error: Option<String>,
}

fn response(
    event: &Event,
    signer: &str,
    client: &str,
    key: &[u8; 32],
    id: &str,
) -> Result<Option<Reply>> {
    if event.kind != 24133 || event.pubkey != signer || !event.tag_values("p").any(|p| p == client)
    {
        return Ok(None);
    }
    event.verify()?;
    let plaintext = nip44::decrypt(key, &event.content)?;
    let response: Response = serde_json::from_str(&plaintext)
        .map_err(|_invalid| Error::network("invalid remote signer response"))?;
    if response.id != id {
        return Ok(None);
    }
    if response.result.as_deref() == Some("auth_url") {
        let url = response
            .error
            .ok_or_else(|| Error::network("remote signer auth_url is missing"))?;
        let parsed = url::Url::parse(&url)
            .map_err(|_invalid| Error::network("invalid remote signer auth_url"))?;
        if !matches!(parsed.scheme(), "https" | "http")
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || url.chars().any(char::is_control)
        {
            return Err(Error::network("invalid remote signer auth_url"));
        }
        return Ok(Some(Reply::Auth(url)));
    }
    if response.error.is_some_and(|e| !e.is_empty()) {
        return Ok(Some(Reply::Rejected));
    }
    response
        .result
        .map(|value| Some(Reply::Value(value)))
        .ok_or_else(|| Error::network("remote signer response has no result"))
}

#[cfg(test)]
#[path = "nip46_tests.rs"]
mod tests;
