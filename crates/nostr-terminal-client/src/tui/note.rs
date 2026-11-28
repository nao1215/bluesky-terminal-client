//! Nostr-specific actions and attachment discovery.
use crate::api::event::{Event, Unsigned, bytes};
use std::collections::BTreeSet;

pub(super) fn draft(kind: u16, content: String, target: Option<&Event>) -> Unsigned {
    let mut tags = Vec::new();
    if let Some(note) = target {
        if kind == 1 {
            let root = note
                .tags
                .iter()
                .find(|t| {
                    t.first().is_some_and(|s| s == "e")
                        && t.get(3).is_some_and(|s| s == "root")
                        && t.get(1).is_some_and(|s| bytes::<32>(s).is_ok())
                })
                .or_else(|| {
                    note.tags.iter().find(|t| {
                        t.first().is_some_and(|s| s == "e")
                            && t.get(3).is_none_or(String::is_empty)
                            && t.get(1).is_some_and(|s| bytes::<32>(s).is_ok())
                    })
                })
                .and_then(|t| t.get(1))
                .map(String::as_str)
                .or_else(|| note.reply_id())
                .unwrap_or(&note.id);
            tags.push(vec!["e".into(), root.into(), String::new(), "root".into()]);
            if root != note.id {
                tags.push(vec![
                    "e".into(),
                    note.id.clone(),
                    String::new(),
                    "reply".into(),
                ]);
            }
        } else {
            tags.push(vec!["e".into(), note.id.clone()]);
        }
        let mut people = BTreeSet::from([note.pubkey.clone()]);
        if kind == 1 {
            people.extend(
                note.tag_values("p")
                    .filter(|p| bytes::<32>(p).is_ok())
                    .map(str::to_owned),
            );
        }
        tags.extend(people.into_iter().map(|p| vec!["p".into(), p]));
        if kind == 7 {
            tags.push(vec!["k".into(), "1".into()]);
        }
    }
    Unsigned {
        created_at: 0,
        kind,
        tags,
        content,
    }
}

pub(super) fn pictures(note: &Event) -> Vec<String> {
    let mut urls = Vec::new();
    for raw in note.content.split_whitespace() {
        let raw = raw.trim_matches(['(', ')', '[', ']', '<', '>', ',', '"']);
        if let Ok(url) = url::Url::parse(raw) {
            let path = url.path().to_ascii_lowercase();
            if [".png", ".jpg", ".jpeg", ".gif", ".webp", ".bmp", ".avif"]
                .iter()
                .any(|ext| path.ends_with(ext))
            {
                add_picture(&mut urls, raw);
            }
        }
    }
    for tag in &note.tags {
        if tag.first().is_some_and(|s| s == "imeta") {
            for field in tag.iter().skip(1) {
                if let Some(url) = field.strip_prefix("url ") {
                    add_picture(&mut urls, url);
                }
            }
        }
    }
    urls
}

fn add_picture(urls: &mut Vec<String>, raw: &str) {
    if let Ok(url) = url::Url::parse(raw)
        && matches!(url.scheme(), "http" | "https")
        && url.host_str().is_some()
        && !urls.iter().any(|u| u == raw)
    {
        urls.push(raw.into());
    }
}
