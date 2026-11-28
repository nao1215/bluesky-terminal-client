//! Shared CLI/TUI operations. All durable writes go through the configured signer.
use super::{
    Client,
    event::{Event, Unsigned, bytes},
    nip19::Identifier,
    relay::{Filter, Outcome},
};
use crate::error::{Error, Result};
use serde_json::Value;

pub(crate) fn public(value: &str) -> Result<String> {
    if bytes::<32>(value).is_ok() {
        return Ok(value.into());
    }
    match Identifier::decode(value.trim_start_matches("nostr:"))? {
        Identifier::Public(k) | Identifier::Profile { public: k, .. } => Ok(hex::encode(k)),
        _ => Err(Error::usage("expected a public key, npub or nprofile")),
    }
}
pub(crate) fn id(value: &str) -> Result<String> {
    if bytes::<32>(value).is_ok() {
        return Ok(value.into());
    }
    match Identifier::decode(value.trim_start_matches("nostr:"))? {
        Identifier::Note(k) | Identifier::Event { id: k, .. } => Ok(hex::encode(k)),
        _ => Err(Error::usage("expected an event ID, note or nevent")),
    }
}
pub(crate) fn draft(kind: u16, tags: Vec<Vec<String>>, content: String) -> Unsigned {
    Unsigned {
        created_at: 0,
        kind,
        tags,
        content,
    }
}
pub(crate) fn tag(name: &str, value: &str) -> Vec<String> {
    vec![name.into(), value.into()]
}
pub(crate) fn now() -> Result<u64> {
    u64::try_from(jiff::Timestamp::now().as_second()).map_err(Error::usage)
}

impl super::relay::AuthSigner for Client<'_> {
    fn sign_auth(&self, event: Unsigned) -> Result<Event> {
        self.sign(event)
    }
}

impl Client<'_> {
    pub(crate) fn sign(&self, mut draft: Unsigned) -> Result<Event> {
        if draft.created_at == 0 {
            draft.created_at = now()?;
        }
        if let Some(bunker) = &self.config.bunker {
            bunker.sign_event(&self.config.public, &draft, self.config.timeout, &|url| {
                self.authorization(url)
            })
        } else {
            Event::sign(
                &self.config.signing_key()?,
                draft.created_at,
                draft.kind,
                draft.tags,
                draft.content,
            )
        }
    }
    pub(crate) fn publish(&self, draft: Unsigned) -> Result<Outcome<Event>> {
        let event = self.sign(draft)?;
        let sent = self.relays().publish(&event, Some(self))?;
        Ok(Outcome {
            value: event,
            answered: sent.value,
            warnings: sent.warnings,
        })
    }
    pub(crate) fn profile(&self, author: &str) -> Result<Outcome<Option<Event>>> {
        self.latest_for(0, author)
    }
    pub(crate) fn latest_for(&self, kind: u16, author: &str) -> Result<Outcome<Option<Event>>> {
        let found = self.relays().query(
            Filter {
                kinds: vec![kind],
                authors: vec![public(author)?],
                ..Filter::default()
            },
            1,
        )?;
        Ok(Outcome {
            value: found.value.into_iter().next(),
            answered: found.answered,
            warnings: found.warnings,
        })
    }
    pub(crate) fn note(&self, value: &str) -> Result<Outcome<Event>> {
        let found = self.relays().query(
            Filter {
                kinds: vec![1, 6, 16],
                ids: vec![id(value)?],
                ..Filter::default()
            },
            1,
        )?;
        Ok(Outcome {
            value: found
                .value
                .into_iter()
                .next()
                .ok_or_else(|| Error::unavailable("event not found on configured relays"))?,
            answered: found.answered,
            warnings: found.warnings,
        })
    }
    pub(crate) fn author(&self, author: &str, count: usize) -> Result<Outcome<Vec<Event>>> {
        self.relays().query(
            Filter {
                kinds: vec![1, 6, 16],
                authors: vec![public(author)?],
                ..Filter::default()
            },
            count,
        )
    }
    pub(crate) fn thread(&self, value: &str, count: usize) -> Result<Outcome<Vec<Event>>> {
        let focal = self.note(value)?;
        let mut notes = vec![focal.value];
        let mut warnings = focal.warnings;
        // Bound cycles and ancestry independently of relay behavior.
        for _ in 0..32 {
            let Some(parent) = notes.last().and_then(Event::reply_id).map(str::to_owned) else {
                break;
            };
            if notes.iter().any(|n| n.id == parent) {
                break;
            }
            match self.note(&parent) {
                Ok(p) => {
                    notes.push(p.value);
                    warnings.extend(p.warnings);
                }
                Err(e) => {
                    warnings.push(e.error);
                    break;
                }
            }
        }
        notes.reverse();
        let mut frontier = notes.iter().map(|e| e.id.clone()).collect::<Vec<_>>();
        let mut seen = frontier
            .iter()
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let mut remaining = count;
        let mut answered = focal.answered;
        for _ in 0..8 {
            if remaining == 0 || frontier.is_empty() {
                break;
            }
            let mut replies = self.relays().query(
                Filter {
                    kinds: vec![1],
                    references: frontier.clone(),
                    ..Filter::default()
                },
                remaining,
            )?;
            replies.value.retain(|e| {
                !seen.contains(&e.id)
                    && e.reply_id()
                        .is_some_and(|id| frontier.iter().any(|p| p == id))
            });
            frontier = replies.value.iter().map(|e| e.id.clone()).collect();
            seen.extend(frontier.iter().cloned());
            remaining = remaining.saturating_sub(replies.value.len());
            notes.extend(replies.value);
            warnings.extend(replies.warnings);
            answered = replies.answered;
        }
        Ok(Outcome {
            value: notes,
            warnings,
            answered,
        })
    }

    pub(crate) fn relation(&self, kind: u16, author: &str, add: bool) -> Result<Unsigned> {
        let author = public(author)?;
        let old = self.latest(kind)?.value;
        let mut tags = old.as_ref().map(|e| e.tags.clone()).unwrap_or_default();
        let present = tags
            .iter()
            .any(|t| t.first().is_some_and(|s| s == "p") && t.get(1) == Some(&author));
        if !add {
            tags.retain(|t| !(t.first().is_some_and(|s| s == "p") && t.get(1) == Some(&author)));
        } else if !present {
            tags.push(tag("p", &author));
        }
        replacement(
            kind,
            tags,
            old.as_ref().map(|e| e.content.clone()).unwrap_or_default(),
            old.as_ref(),
        )
    }
    pub(crate) fn update_profile(&self, patch: Value) -> Result<Unsigned> {
        let old = self.latest(0)?.value;
        let mut metadata = match &old {
            Some(e) => serde_json::from_str::<Value>(&e.content).map_err(|_invalid| {
                Error::network("existing profile is invalid JSON; refusing to discard fields")
            })?,
            None => serde_json::json!({}),
        };
        let object = metadata
            .as_object_mut()
            .ok_or_else(|| Error::network("profile must be a JSON object"))?;
        object.extend(
            patch
                .as_object()
                .ok_or_else(|| Error::usage("profile patch must be an object"))?
                .clone(),
        );
        let mut tags = old.as_ref().map(|e| e.tags.clone()).unwrap_or_default();
        tags.retain(|t| {
            t.first().is_none_or(|s| s != "imeta")
                || t.iter()
                    .filter_map(|s| s.strip_prefix("url "))
                    .any(|u| metadata["picture"] == u || metadata["banner"] == u)
        });
        replacement(0, tags, metadata.to_string(), old.as_ref())
    }
    pub(crate) fn retract(&self, target: &Event, kind: u16) -> Result<Option<Unsigned>> {
        let mut found = self.active_actions(vec![target.id.clone()])?;
        found
            .value
            .retain(|e| e.kind == kind || (kind == 6 && e.kind == 16));
        if found.value.is_empty() {
            return Ok(None);
        }
        Ok(Some(deletion(&found.value, &self.config.public, "")?))
    }
}
pub(super) fn replacement(
    kind: u16,
    tags: Vec<Vec<String>>,
    content: String,
    old: Option<&Event>,
) -> Result<Unsigned> {
    let mut time = now()?;
    if let Some(old) = old {
        if old.created_at > time.saturating_add(2) {
            return Err(Error::network(
                "latest list/profile is future-dated; refusing a lossy replacement",
            ));
        }
        if old.created_at >= time {
            std::thread::sleep(std::time::Duration::from_secs(old.created_at - time + 1));
            time = now()?;
        }
    }
    Ok(Unsigned {
        created_at: time,
        kind,
        tags,
        content,
    })
}
pub(crate) fn deletion(events: &[Event], owner: &str, reason: &str) -> Result<Unsigned> {
    if events.iter().any(|e| e.pubkey != owner) {
        return Err(Error::usage("only your own events can be deleted"));
    }
    let mut tags = Vec::new();
    for e in events {
        tags.push(tag("e", &e.id));
        let k = tag("k", &e.kind.to_string());
        if !tags.contains(&k) {
            tags.push(k);
        }
    }
    Ok(draft(5, tags, reason.into()))
}
pub(crate) fn reaction(note: &Event, content: &str) -> Unsigned {
    let mut tags = vec![
        tag("e", &note.id),
        tag("p", &note.pubkey),
        tag("k", &note.kind.to_string()),
    ];
    let mut text = content;
    if let Some((shortcode, url)) = content.split_once(' ')
        && let Some(name) = shortcode
            .strip_prefix(':')
            .and_then(|s| s.strip_suffix(':'))
        && !name.is_empty()
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-')
        && super::media::valid_url(url)
    {
        tags.push(vec!["emoji".into(), name.into(), url.into()]);
        text = shortcode;
    }
    draft(7, tags, text.into())
}

pub(crate) fn repost(note: &Event, relay: &str) -> Result<Unsigned> {
    let kind = if note.kind == 1 { 6 } else { 16 };
    let mut tags = vec![
        vec!["e".into(), note.id.clone(), relay.into()],
        tag("p", &note.pubkey),
    ];
    if kind == 16 {
        tags.push(tag("k", &note.kind.to_string()));
    }
    Ok(draft(
        kind,
        tags,
        if note
            .tags
            .iter()
            .any(|t| t.first().is_some_and(|s| s == "-"))
        {
            String::new()
        } else {
            serde_json::to_string(note).map_err(Error::usage)?
        },
    ))
}
pub(crate) fn report(note: &Event, reason: &str, comment: &str) -> Result<Unsigned> {
    if ![
        "nudity",
        "malware",
        "profanity",
        "illegal",
        "spam",
        "impersonation",
        "other",
    ]
    .contains(&reason)
    {
        return Err(Error::usage("unknown NIP-56 report reason"));
    }
    Ok(draft(
        1984,
        vec![
            vec!["p".into(), note.pubkey.clone(), reason.into()],
            vec!["e".into(), note.id.clone(), reason.into()],
        ],
        comment.into(),
    ))
}
pub(crate) fn post(
    text: String,
    reply: Option<&Event>,
    quote: Option<&Event>,
    cw: Option<&str>,
    relay: &str,
) -> Unsigned {
    let mut tags = Vec::new();
    if let Some(e) = reply {
        let root = e
            .tags
            .iter()
            .find(|t| t.first().is_some_and(|s| s == "e") && t.get(3).is_some_and(|s| s == "root"))
            .and_then(|t| t.get(1))
            .filter(|s| bytes::<32>(s).is_ok())
            .map_or_else(|| e.reply_id().unwrap_or(&e.id), String::as_str);
        tags.push(vec!["e".into(), root.into(), relay.into(), "root".into()]);
        if root != e.id {
            tags.push(vec!["e".into(), e.id.clone(), relay.into(), "reply".into()]);
        }
        let mut people = std::collections::BTreeSet::from([e.pubkey.as_str()]);
        people.extend(e.tag_values("p").filter(|s| bytes::<32>(s).is_ok()));
        tags.extend(people.into_iter().map(|p| tag("p", p)));
    }
    if let Some(e) = quote {
        tags.push(vec![
            "q".into(),
            e.id.clone(),
            relay.into(),
            e.pubkey.clone(),
        ]);
        tags.push(tag("p", &e.pubkey));
    }
    if let Some(warning) = cw {
        tags.push(tag("content-warning", warning));
    }
    draft(1, tags, text)
}

impl Client<'_> {
    /// Observed reactions/reposts, excluding this author's deletion requests.
    pub(crate) fn active_actions(&self, ids: Vec<String>) -> Result<Outcome<Vec<Event>>> {
        let mut found = self.relays().query(
            Filter {
                kinds: vec![6, 7, 16],
                authors: vec![self.config.public.clone()],
                references: ids,
                ..Filter::default()
            },
            1000,
        )?;
        if found.value.is_empty() {
            return Ok(found);
        }
        let deleted = self.relays().query(
            Filter {
                kinds: vec![5],
                authors: vec![self.config.public.clone()],
                references: found.value.iter().map(|e| e.id.clone()).collect(),
                ..Filter::default()
            },
            1000,
        )?;
        let ids: std::collections::HashSet<_> = deleted
            .value
            .iter()
            .flat_map(|e| e.tag_values("e").map(str::to_owned))
            .collect();
        found.value.retain(|e| !ids.contains(&e.id));
        found.warnings.extend(deleted.warnings);
        Ok(found)
    }
}

impl Client<'_> {
    pub(crate) fn visible(&self, out: Outcome<Vec<Event>>) -> Result<Outcome<Vec<Event>>> {
        Ok(filter_muted(out, self.latest(10000)))
    }
}

/// Apply an independently fetched mute list before publishing a page to the UI.
pub(crate) fn filter_muted(
    mut out: Outcome<Vec<Event>>,
    muted: Result<Outcome<Option<Event>>>,
) -> Outcome<Vec<Event>> {
    let muted = match muted {
        Ok(out) => out,
        Err(e) => {
            out.warnings
                .push(format!("mute list unavailable: {}", e.error));
            return out;
        }
    };
    if let Some(list) = muted.value {
        let people: std::collections::HashSet<_> = list.tag_values("p").collect();
        let events: std::collections::HashSet<_> = list.tag_values("e").collect();
        let words: Vec<_> = list.tag_values("word").map(str::to_lowercase).collect();
        let hashtags: std::collections::HashSet<_> = list.tag_values("t").collect();
        out.value.retain(|e| {
            !people.contains(e.pubkey.as_str())
                && !events.contains(e.id.as_str())
                && !e.tag_values("e").any(|id| events.contains(id))
                && !words.iter().any(|w| e.content.to_lowercase().contains(w))
                && !e.tag_values("t").any(|t| hashtags.contains(t))
        });
    }
    out.warnings.extend(muted.warnings);
    out
}

/// Only render an embedded note after checking its identity and signature.
pub(crate) fn reposted(event: &Event) -> Option<Event> {
    if !matches!(event.kind, 6 | 16) {
        return None;
    }
    let inner: Event = serde_json::from_str(&event.content).ok()?;
    (inner.kind == 1 && event.tag_values("e").any(|id| id == inner.id) && inner.verify().is_ok())
        .then_some(inner)
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::{
        elliptic_curve::Generate,
        schnorr::SigningKey,
        sha2::{Digest, Sha256},
    };
    use proptest::prelude::*;
    fn note() -> Event {
        Event::sign(&SigningKey::generate(), 12, 1, vec![], "hello".into()).unwrap()
    }
    #[test]
    fn nip_tags_ownership_and_quotes_are_independent_of_replies() {
        let e = note();
        let repost = repost(&e, "wss://relay.invalid/").unwrap();
        assert_eq!(
            repost.tags,
            vec![
                vec!["e", &e.id, "wss://relay.invalid/"],
                vec!["p", &e.pubkey]
            ]
        );
        assert_eq!(serde_json::from_str::<Event>(&repost.content).unwrap(), e);
        let quote = post(
            "quote".into(),
            None,
            Some(&e),
            Some("spoiler"),
            "wss://relay.invalid/",
        );
        assert!(!quote.tags.iter().any(|t| t[0] == "e"));
        assert!(quote.tags.contains(&vec![
            "q".into(),
            e.id.clone(),
            "wss://relay.invalid/".into(),
            e.pubkey.clone()
        ]));
        assert!(deletion(std::slice::from_ref(&e), &"00".repeat(32), "").is_err());
        let delete = deletion(std::slice::from_ref(&e), &e.pubkey, "reason").unwrap();
        assert_eq!(delete.tags, vec![vec!["e", &e.id], vec!["k", "1"]]);
        assert_eq!(
            report(&e, "spam", "why").unwrap().tags,
            vec![vec!["p", &e.pubkey, "spam"], vec!["e", &e.id, "spam"]]
        );
        assert!(report(&e, "unknown", "").is_err());
    }
    proptest! {
        #[test]
        fn every_new_kind_has_canonical_id_valid_signature_and_tamper_detection(kind in prop::sample::select(vec![0u16,1,3,5,6,7,13,14,16,1059,1984,10000,10050,24242]), content in ".{0,128}", values in prop::collection::vec(".{0,30}",0..5)) {
            let key=SigningKey::generate();
            let tags=values.into_iter().map(|v|tag("p",&v)).collect::<Vec<_>>();
            let mut e=Event::sign(&key,123,kind,tags.clone(),content.clone()).unwrap();
            let canonical=serde_json::json!([0,e.pubkey,123,kind,tags,content]).to_string();
            prop_assert_eq!(&e.id,&hex::encode(Sha256::digest(canonical.as_bytes())));
            prop_assert!(e.verify().is_ok());
            let round:Event=serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
            prop_assert_eq!(&e,&round);
            e.tags.push(tag("altered","yes"));prop_assert!(e.verify().is_err());
        }
    }
}
