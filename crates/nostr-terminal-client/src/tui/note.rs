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
        if tag.first().is_some_and(|s| s == "imeta")
            && !tag.iter().any(|field| field.starts_with("m video/"))
        {
            for field in tag.iter().skip(1) {
                if let Some(url) = field.strip_prefix("url ")
                    && !video_url(url)
                {
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

fn video_url(raw: &str) -> bool {
    url::Url::parse(raw).is_ok_and(|url| {
        [".mp4", ".webm", ".mov", ".mkv", ".m4v", ".avi"]
            .iter()
            .any(|ext| url.path().to_ascii_lowercase().ends_with(ext))
    })
}

pub(super) fn has_video(note: &Event) -> bool {
    note.content.split_whitespace().any(video_url)
        || note.tags.iter().any(|tag| {
            tag.first().is_some_and(|s| s == "imeta")
                && tag.iter().any(|field| {
                    field.starts_with("m video/")
                        || field.strip_prefix("url ").is_some_and(video_url)
                })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use k256::{elliptic_curve::Generate, schnorr::SigningKey};

    #[test]
    fn video_imeta_is_not_sent_to_the_picture_decoder() {
        let event = Event::sign(
            &SigningKey::generate(),
            1,
            1,
            vec![
                vec![
                    "imeta".into(),
                    "url http://127.0.0.1/opaque".into(),
                    "m video/mp4".into(),
                ],
                vec![
                    "imeta".into(),
                    "url http://127.0.0.1/picture.png".into(),
                    "m image/png".into(),
                ],
            ],
            "http://127.0.0.1/video.webm".into(),
        )
        .unwrap();
        assert!(has_video(&event));
        assert_eq!(pictures(&event), ["http://127.0.0.1/picture.png"]);
    }
}
