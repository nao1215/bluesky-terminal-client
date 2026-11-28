#![no_main]

include!("common.rs");
use libfuzzer_sys::fuzz_target;

fuzz_target!(|text: &str| {
    if let Ok(message) = relay::RelayMessage::parse(text) {
        let value: serde_json::Value = serde_json::from_str(text).unwrap();
        assert_eq!(
            relay::RelayMessage::parse(&value.to_string()).unwrap(),
            message
        );
        if let relay::RelayMessage::Event(_, event) = message {
            let _ = event.verify();
            let _ = event.reply_id();
        }
    }
});
