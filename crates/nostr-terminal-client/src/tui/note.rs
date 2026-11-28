//! Nostr-specific actions and attachment discovery.
use crate::api::event::{Event, Unsigned};

pub(super) fn draft(kind: u16, content: String, target: Option<&Event>) -> Unsigned {
    use crate::api::actions;
    if kind == 1 {
        return actions::post(content, target, None, None, "");
    }
    if kind == 7
        && let Some(e) = target
    {
        return actions::reaction(e, &content);
    }
    let tags = target
        .map(|e| {
            vec![
                vec!["e".into(), e.id.clone(), String::new()],
                vec!["p".into(), e.pubkey.clone()],
            ]
        })
        .unwrap_or_default();
    actions::draft(kind, tags, content)
}

pub(super) fn attachments(note: &Event) -> Vec<String> {
    crate::api::media::files(note)
        .iter()
        .filter_map(|f| f["url"].as_str().map(str::to_owned))
        .collect()
}
pub(super) fn pictures(note: &Event) -> Vec<String> {
    crate::api::media::files(note)
        .iter()
        .filter(|f| f["type"] != "video" && !f["url"].as_str().is_some_and(video))
        .filter_map(|f| f["url"].as_str().map(str::to_owned))
        .collect()
}
pub(super) fn video(raw: &str) -> bool {
    crate::api::media::is_video(raw)
}
pub(super) fn has_video(note: &Event) -> bool {
    crate::api::media::files(note)
        .iter()
        .any(|f| f["type"] == "video" || f["url"].as_str().is_some_and(video))
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
