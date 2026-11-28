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
        "user": user(&event.pubkey)?, "text": if matches!(event.kind,6|16) { "" } else { &event.content }, "cw": event.tags.iter().find(|t|t.first().is_some_and(|s|s=="content-warning")).map(|t|t.get(1).map_or("",String::as_str)),
        "likeCount": null, "repostCount": null, "repliesCount": null, "files": if matches!(event.kind,6|16) { vec![] } else { crate::api::media::files(event) },
        "replyId": event.reply_id(), "quoteId": event.tag_values("q").next(), "repostOf": repost_of(event)?,
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
    let action = matches!(event.kind, 6 | 7 | 16);
    let target = if event.kind == 3 {
        None
    } else if action {
        event.tag_values("e").last()
    } else {
        Some(event.id.as_str())
    };
    let post = if event.kind == 3 {
        Value::Null
    } else if action {
        repost_of(event)?
    } else {
        post(event)?
    };
    Ok(json!({"id": event.id, "createdAt": timestamp(event)?,
        "type": match event.kind { 3 => "follow", 7 => "like", 6 | 16 => "repost", _ if event.tag_values("q").next().is_some() => "quote", _ if event.reply_id().is_some() => "reply", _ => "mention" },
        "reaction": (event.kind == 7).then_some(&event.content), "user": user(&event.pubkey)?, "postId": target, "post": post }))
}

pub(crate) fn text(value: &Value) -> String {
    if value.get("action").is_some() && value["action"] != "post" {
        return format!("{value}\n");
    }
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
    let written = if json {
        terminal_kit::output::write_json_line(out, &value.to_string())
    } else {
        terminal_kit::output::write_text(out, &terminal_kit::text::drawable(&text(value)))
    };
    match written {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(false),
        Err(e) => Err(Error::file(format!("cannot write output: {e}"))),
    }
}

fn repost_of(event: &Event) -> Result<Value> {
    crate::api::actions::reposted(event)
        .as_ref()
        .map(post)
        .transpose()
        .map(|v| v.unwrap_or(Value::Null))
}
