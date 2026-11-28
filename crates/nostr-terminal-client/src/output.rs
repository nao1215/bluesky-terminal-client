//! The common JSON contract and terminal-safe text rendering.

use crate::{
    api::{
        event::{Event, bytes},
        nip19::Identifier,
    },
    error::{Error, Result},
};
use serde_json::{Value, json};
use std::io::{self, Write};

pub(crate) fn user(public: &str) -> Result<Value> {
    let key = bytes(public)?;
    Ok(
        json!({"id": public, "username": Identifier::Public(key).encode()?, "host": null, "name": null}),
    )
}

pub(crate) fn account(public: &str, metadata: Option<&Event>) -> Result<Value> {
    let mut value = user(public)?;
    let profile = metadata
        .and_then(|e| serde_json::from_str::<Value>(&e.content).ok())
        .unwrap_or(Value::Null);
    let text = |key: &str| profile.get(key).and_then(Value::as_str);
    value["name"] = json!(text("display_name").or_else(|| text("name")));
    value["url"] = Value::Null;
    value["description"] = json!(text("about"));
    for field in [
        "followersCount",
        "followingCount",
        "postsCount",
        "following",
    ] {
        value[field] = Value::Null;
    }
    value["npub"] = value["username"].clone();
    value["nprofile"] = json!(
        Identifier::Profile {
            public: bytes(public)?,
            relays: Vec::new()
        }
        .encode()?
    );
    value["picture"] = json!(text("picture"));
    Ok(value)
}

pub(crate) fn timestamp(event: &Event) -> Result<String> {
    let seconds = i64::try_from(event.created_at).map_err(Error::network)?;
    jiff::Timestamp::from_second(seconds)
        .map(|t| t.to_string())
        .map_err(Error::network)
}

pub(crate) fn post(event: &Event) -> Result<Value> {
    let id = bytes(&event.id)?;
    let note = Identifier::Note(id).encode()?;
    Ok(json!({
        "id": event.id, "url": null, "uri": format!("nostr:{note}"), "createdAt": timestamp(event)?,
        "user": user(&event.pubkey)?, "text": event.content, "cw": event.tag_values("content-warning").next(),
        "likeCount": null, "repostCount": null, "repliesCount": null, "files": [],
        "replyId": event.reply_id(), "quoteId": event.tag_values("q").next(), "repostOf": null,
        "kind": event.kind, "tags": event.tags, "sig": event.sig, "note": note,
        "nevent": Identifier::Event { id, relays: Vec::new(), author: Some(bytes(&event.pubkey)?), kind: Some(u32::from(event.kind)) }.encode()?
    }))
}

pub(crate) fn publication(event: &Event, accepted: usize) -> Result<Value> {
    let note = Identifier::Note(bytes(&event.id)?).encode()?;
    Ok(
        json!({"action": "post", "target": null, "id": event.id, "url": null,
        "already": false, "uri": format!("nostr:{note}"), "acceptedRelays": accepted}),
    )
}

pub(crate) fn notification(event: &Event) -> Result<Value> {
    Ok(json!({"id": event.id, "createdAt": timestamp(event)?,
        "type": match event.kind { 3 => "follow", 6 => "repost", 7 => "reaction", _ if event.reply_id().is_some() => "reply", _ => "mention" },
        "user": user(&event.pubkey)?, "postId": event.id, "post": post(event)? }))
}

pub(crate) fn text(value: &Value) -> String {
    if value["action"] == "post" {
        return format!("{}\n", value["uri"].as_str().unwrap_or(""));
    }
    if value.get("post").is_some() {
        return format!(
            "{}: {}",
            value["type"].as_str().unwrap_or("mention"),
            text(&value["post"])
        );
    }
    if let Some(content) = value["text"].as_str() {
        return format!(
            "{} · {}\n{content}\n{}\n",
            value["user"]["username"].as_str().unwrap_or(""),
            terminal_kit::clock::local_time(value["createdAt"].as_str().unwrap_or("")),
            value["uri"].as_str().unwrap_or("")
        );
    }
    format!(
        "{}\n{}\n{}\n",
        value["username"].as_str().unwrap_or(""),
        value["name"].as_str().unwrap_or(""),
        value["description"].as_str().unwrap_or("")
    )
}

pub(crate) fn emit(out: &mut dyn Write, value: &Value, json: bool) -> Result<bool> {
    let line = if json {
        format!("{value}\n")
    } else {
        terminal_kit::text::drawable(&text(value)).into_owned()
    };
    match out.write_all(line.as_bytes()) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(e) => Err(Error::file(format!("cannot write output: {e}"))),
    }
}
