#![no_main]

include!("common.rs");
use libfuzzer_sys::fuzz_target;
#[path = "../../src/tui/note.rs"]
mod note;

fuzz_target!(|data: &[u8]| {
    if let Ok(event) = serde_json::from_slice::<event::Event>(data) {
        let encoded = serde_json::to_vec(&event).unwrap();
        assert_eq!(
            serde_json::from_slice::<event::Event>(&encoded).unwrap(),
            event
        );
        let pictures = note::pictures(&event);
        let unique: std::collections::BTreeSet<_> = pictures.iter().collect();
        assert_eq!(pictures.len(), unique.len());
        for picture in &pictures {
            let url = url::Url::parse(picture).unwrap();
            assert!(matches!(url.scheme(), "http" | "https"));
            assert!(url.host_str().is_some());
        }
        for kind in [1, 6, 7] {
            let draft = note::draft(kind, event.content.clone(), Some(&event));
            assert_eq!(draft.kind, kind);
            assert_eq!(draft.content, event.content);
        }
        let _ = event.verify();
        let digest = event.digest().unwrap();
        let mut changed = event.clone();
        changed.content.push('x');
        assert_ne!(digest, changed.digest().unwrap());
        for name in ["e", "p", "imeta", "content-warning", ""] {
            assert_eq!(
                event.tag_values(name).count(),
                event
                    .tags
                    .iter()
                    .filter(|t| t.len() >= 2 && t[0] == name)
                    .count()
            );
        }
        if let Some(reply) = event.reply_id() {
            assert!(event::bytes::<32>(reply).is_ok());
            assert!(event.tag_values("e").any(|id| id == reply));
        }
    }
});
