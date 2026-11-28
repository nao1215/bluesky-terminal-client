//! Optional relay information, fetched once per URL for the process lifetime.

use serde::Deserialize;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub(super) struct Limits {
    pub(super) supported_nips: Vec<u16>,
    pub(super) max_subscriptions: Option<usize>,
    pub(super) max_limit: Option<usize>,
    pub(super) max_message_length: Option<usize>,
    pub(super) auth_required: bool,
    pub(super) payment_required: bool,
}

#[derive(Default, Deserialize)]
#[serde(default)]
struct Information {
    supported_nips: Vec<u16>,
    limitation: Limits,
}

pub(super) fn limits(address: &str, timeout: Duration) -> Limits {
    type Cache = Mutex<HashMap<String, Arc<OnceLock<Limits>>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    let entry = CACHE
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .entry(address.into())
        .or_default()
        .clone();
    entry
        .get_or_init(|| fetch(address, timeout).unwrap_or_default())
        .clone()
}

fn fetch(address: &str, timeout: Duration) -> Option<Limits> {
    let mut url = url::Url::parse(address).ok()?;
    let scheme = if url.scheme() == "wss" {
        "https"
    } else {
        "http"
    };
    url.set_scheme(scheme).ok()?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .proxy(None)
        .max_redirects(0)
        .timeout_global(Some(timeout.min(Duration::from_secs(2))))
        .build()
        .into();
    let mut response = agent
        .get(url.as_str())
        .header("Accept", "application/nostr+json")
        .header("Connection", "close")
        .call()
        .ok()?;
    if response.status() != 200 {
        return None;
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(65536)
        .read_to_vec()
        .ok()?;
    serde_json::from_slice::<Information>(&bytes)
        .ok()
        .map(|mut info| {
            info.limitation.supported_nips = info.supported_nips;
            info.limitation
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_limits_and_unknown_fields() {
        let info: Information = serde_json::from_str(r#"{"supported_nips":[1,11],"limitation":{"max_subscriptions":1,"max_limit":2,"max_message_length":512,"auth_required":true,"payment_required":true,"future":4}}"#).unwrap();
        assert_eq!(info.limitation.max_subscriptions, Some(1));
        assert_eq!(info.limitation.max_limit, Some(2));
        assert_eq!(info.limitation.max_message_length, Some(512));
        assert!(info.limitation.auth_required && info.limitation.payment_required);
        assert_eq!(
            serde_json::from_str::<Information>("{}")
                .unwrap()
                .limitation
                .max_limit,
            None
        );
        assert!(serde_json::from_str::<Information>(r#"{"limitation":{"max_limit":-1}}"#).is_err());
    }
}
