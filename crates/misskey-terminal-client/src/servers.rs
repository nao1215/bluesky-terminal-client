//! The servers msky offers to log in to: the list Misskey Hub publishes
//! (the one join.misskey.page shows), else a short one msky carries.
//!
//! msky is for Misskey itself. The Hub's list is of servers that run it,
//! and msky keeps the ones whose nodeinfo says `misskey`; a fork may work,
//! but it is not looked after. Any server can still be named with --host.

use serde::Deserialize;
use serde_json::Value;

use crate::error::{Error, Result};

/// Where Misskey Hub publishes its list of servers.
const HUB_URL: &str = "https://instanceapp.misskey.page/instances.json";

/// Environment variable naming another address for the list, or `off` for
/// the built-in one alone: the tests' stand-in servers use it, so no test
/// reaches Misskey Hub.
const SERVERS_URL_ENV: &str = "MSKY_SERVERS_URL";

/// The largest list read; the Hub's is about 5 MB.
const MAX_LIST_BYTES: u64 = 32 * 1024 * 1024;

/// A server to log in to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Server {
    /// Its host, `misskey.io`.
    pub host: String,
    /// Its name, as its `meta` gives it.
    pub name: String,
    /// How many accounts it has, as its nodeinfo says.
    pub users: Option<u64>,
    /// The languages it is for.
    pub langs: Vec<String>,
    /// Whether it takes new accounts.
    pub open: Option<bool>,
}

/// Large servers that run Misskey itself, with the names their `meta`
/// gave on 2026-11-27: what is offered when the Hub's list cannot be read.
const BUILTIN: &[(&str, &str)] = &[
    ("misskey.io", "Misskey.io"),
    ("misskey-square.net", "みすきーすくえあ！"),
    ("takusuki.com", "TRPGがすきー！"),
    ("oekakiskey.com", "お絵かきすきー"),
    ("misskey.day", "ふじさんすきー【misskey.day】"),
];

/// The built-in list.
pub(crate) fn builtin() -> Vec<Server> {
    BUILTIN
        .iter()
        .map(|(host, name)| Server {
            host: (*host).to_string(),
            name: (*name).to_string(),
            users: None,
            langs: vec!["ja".into()],
            open: None,
        })
        .collect()
}

/// An entry of the Hub's `instancesInfos`.
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct HubEntry {
    url: String,
    name: Option<String>,
    langs: Vec<String>,
    nodeinfo: Option<Value>,
}

/// The servers of the Hub's list `v` that run Misskey, the most accounts
/// first. An entry that cannot be read is left out.
fn from_hub(v: &Value) -> Vec<Server> {
    let Some(items) = v.get("instancesInfos").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut servers: Vec<Server> = items
        .iter()
        .filter_map(|e| HubEntry::deserialize(e).ok())
        .filter(|e| !e.url.is_empty() && !e.url.contains(['/', ' ']))
        .filter(|e| {
            e.nodeinfo
                .as_ref()
                .and_then(|n| n.pointer("/software/name"))
                .and_then(Value::as_str)
                .is_none_or(|s| s == "misskey")
        })
        .map(|e| {
            let ni = e.nodeinfo.as_ref();
            Server {
                name: crate::text::one_line(e.name.as_deref().unwrap_or(&e.url)),
                users: ni
                    .and_then(|n| n.pointer("/usage/users/total"))
                    .and_then(Value::as_u64),
                open: ni
                    .and_then(|n| n.get("openRegistrations"))
                    .and_then(Value::as_bool),
                host: e.url.to_ascii_lowercase(),
                langs: e.langs,
            }
        })
        .collect();
    servers.sort_by(|a, b| b.users.cmp(&a.users).then_with(|| a.host.cmp(&b.host)));
    servers.dedup_by(|a, b| a.host == b.host);
    servers
}

/// The Hub's list, read from `url`.
pub(crate) fn fetch(agent: &ureq::Agent, url: &str) -> Result<Vec<Server>> {
    let failed = |e: String| {
        Error::api(format!("cannot read the server list at {url}: {e}"))
            .with_hint("give the server with --host")
    };
    let mut resp = agent.get(url).call().map_err(|e| failed(e.to_string()))?;
    if !resp.status().is_success() {
        return Err(failed(format!("HTTP {}", resp.status().as_u16())));
    }
    let text = resp
        .body_mut()
        .with_config()
        .limit(MAX_LIST_BYTES)
        .read_to_string()
        .map_err(|e| failed(e.to_string()))?;
    let v: Value = serde_json::from_str(&text).map_err(|e| failed(e.to_string()))?;
    let servers = from_hub(&v);
    if servers.is_empty() {
        return Err(failed("it lists no server".into()));
    }
    Ok(servers)
}

/// The servers to offer: the Hub's list (or the one `MSKY_SERVERS_URL`
/// names), else the built-in one with why the other could not be read.
pub(crate) fn list() -> (Vec<Server>, Option<Error>) {
    let url = match std::env::var(SERVERS_URL_ENV) {
        Ok(v) if v == "off" => return (builtin(), None),
        Ok(v) if !v.trim().is_empty() => v,
        _ => HUB_URL.to_string(),
    };
    match fetch(&crate::api::agent(), &url) {
        Ok(s) => (s, None),
        Err(e) => (builtin(), Some(e)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::stub_tests::{reply, serve};
    use serde_json::json;
    use std::sync::Arc;

    // Only Misskey's servers are kept, the most accounts first; an entry
    // without a host, or with what is not a host, is left out.
    #[test]
    fn the_hubs_list_is_misskeys_servers_the_largest_first() {
        let v = json!({"instancesInfos": [
            {"url": "small.test", "name": "小さい", "langs": ["ja"],
             "nodeinfo": {"software": {"name": "misskey"}, "usage": {"users": {"total": 10}}, "openRegistrations": true}},
            {"url": "Big.Test", "name": "大きい 🐈\n二行目", "langs": ["ja", "en"],
             "nodeinfo": {"software": {"name": "misskey"}, "usage": {"users": {"total": 9000}}, "openRegistrations": false}},
            {"url": "fork.test", "name": "fork", "nodeinfo": {"software": {"name": "sharkey"}}},
            {"url": "", "name": "no host"},
            {"url": "bad.test/path", "name": "path"},
            {"url": "quiet.test"},
            "not an entry"
        ]});
        let got = from_hub(&v);
        let hosts: Vec<&str> = got.iter().map(|s| s.host.as_str()).collect();
        assert_eq!(hosts, ["big.test", "small.test", "quiet.test"]);
        assert_eq!(got[0].name, "大きい 🐈 二行目");
        assert_eq!(got[0].users, Some(9000));
        assert_eq!(got[0].open, Some(false));
        assert_eq!(got[2].name, "quiet.test");
        assert!(from_hub(&json!({"x": 1})).is_empty());
    }

    #[test]
    fn a_list_that_cannot_be_read_says_where_and_what_to_do() {
        let (url, _) = serve(Arc::new(|_, _, _| reply(503, "busy")));
        let e = fetch(&crate::api::agent(), &format!("{url}/instances.json")).unwrap_err();
        assert!(e.message().contains("HTTP 503"), "{e:?}");
        assert_eq!(e.hint(), Some("give the server with --host"));
        let (url, _) = serve(Arc::new(|_, _, _| reply(200, r#"{"instancesInfos": []}"#)));
        let e = fetch(&crate::api::agent(), &url).unwrap_err();
        assert!(e.message().ends_with("it lists no server"), "{e:?}");
    }

    #[test]
    fn the_builtin_list_is_hosts_with_names() {
        let b = builtin();
        assert_eq!(b[0].host, "misskey.io");
        for s in &b {
            assert!(crate::api::normalize_host(&s.host).is_ok(), "{}", s.host);
            assert!(!s.name.is_empty());
        }
    }
}
