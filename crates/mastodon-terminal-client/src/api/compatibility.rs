//! Endpoint evidence, with optional public server metadata for diagnostics.
//! A software version is an identity, never a capability declaration.

use super::{Answer, Client};
use crate::error::Error;
use serde_json::Value;

/// A 404 on a resource can mean absent, private or deleted. Only collection
/// routes with no resource lookup are unambiguous enough to classify here.
pub(super) fn unsupported_endpoint(path: &str, status: u16) -> bool {
    status == 501
        || (status == 404
            && matches!(
                path,
                "v1/timelines/home"
                    | "v1/timelines/public"
                    | "v1/notifications"
                    | "v2/search"
                    | "v2/media"
                    | "v1/conversations"
                    | "v1/lists"
                    | "v1/followed_tags"
                    | "v1/accounts/update_credentials"
            ))
}

/// Only follow a NodeInfo discovery link on this origin. Metadata never
/// receives the account token, and redirects are disabled by the agent.
fn nodeinfo_url<'a>(origin: &str, discovery: &'a Value) -> Option<&'a str> {
    discovery.get("links")?.as_array()?.iter().find_map(|link| {
        let rel = link.get("rel")?.as_str()?;
        let url = link.get("href")?.as_str()?;
        (matches!(
            rel,
            "http://nodeinfo.diaspora.software/ns/schema/2.0"
                | "http://nodeinfo.diaspora.software/ns/schema/2.1"
        ) && url
            .strip_prefix(origin)
            .is_some_and(|tail| tail.starts_with('/') && !tail.contains(['\\', '#'])))
        .then_some(url)
    })
}

fn server_label(host: &str, instance: &Value, nodeinfo: &Value) -> String {
    let software = &nodeinfo["software"];
    let name = software["name"].as_str().filter(|s| !s.is_empty());
    let version = software["version"]
        .as_str()
        .or_else(|| instance["version"].as_str());
    let identity = match (name, version) {
        (Some(n), Some(v)) => format!("{n} {v}"),
        (Some(n), None) => n.to_string(),
        (None, Some(v)) => format!("reported version {v}"),
        (None, None) => return host.to_string(),
    };
    format!("{host} ({})", crate::text::one_line(&identity))
}

impl Client {
    /// Probe the documented quote fields before any upload or status write.
    pub(crate) fn require_quotes(&self, status: Option<&str>) -> super::Result<&'static str> {
        let available = if let Some(id) = status {
            if self.status(id)?.quote_approval.is_some() {
                true
            } else if self.limits().extension_quotes {
                return Ok("quote_id");
            } else {
                false
            }
        } else {
            self.get_one("v1/accounts/verify_credentials", &[])?["source"]["quote_policy"]
                .is_string()
        };
        if available {
            Ok("quoted_status_id")
        } else {
            Err(self.unsupported(if status.is_some() {
                "quote posts"
            } else {
                "quote approval policies"
            }))
        }
    }

    /// Metadata is best effort: a discovery failure must not replace the
    /// actual endpoint's failure, nor be interpreted as a missing feature.
    fn public_document(&self, url: &str) -> Option<Value> {
        match self.exchange("server metadata", false, &|| self.agent.get(url).call()) {
            Ok(Answer::Took { value, .. }) => Some(value),
            Ok(Answer::Refused(_)) | Err(_) => None,
        }
    }

    pub(super) fn unsupported(&self, feature: &str) -> Error {
        let instance = self
            .public_document(&format!("{}/api/v2/instance", self.origin))
            .or_else(|| self.public_document(&format!("{}/api/v1/instance", self.origin)))
            .unwrap_or(Value::Null);
        let discovery = self
            .public_document(&format!("{}/.well-known/nodeinfo", self.origin))
            .unwrap_or(Value::Null);
        let nodeinfo = nodeinfo_url(&self.origin, &discovery)
            .and_then(|url| self.public_document(url))
            .unwrap_or(Value::Null);
        Error::unsupported(feature, &server_label(self.host(), &instance, &nodeinfo))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::Limits;
    use serde_json::json;

    #[test]
    fn only_endpoint_absence_is_unsupported() {
        for path in [
            "v1/timelines/public",
            "v2/media",
            "v2/search",
            "v1/conversations",
            "v1/lists",
            "v1/followed_tags",
            "v1/accounts/update_credentials",
        ] {
            assert!(unsupported_endpoint(path, 404));
            assert!(unsupported_endpoint(path, 501));
            for status in [400, 401, 403, 422, 429, 500, 502, 503] {
                assert!(!unsupported_endpoint(path, status));
            }
        }
        for path in [
            // Creation and reports can reference missing/hidden resources too.
            "v1/statuses",
            "v1/reports",
            "v1/statuses/123",
            "v1/statuses/123/context",
            "v1/statuses/123/favourite",
            "v1/accounts/lookup",
            "v1/accounts/123",
            "v1/timelines/tag/unknown",
            "v1/media/123",
        ] {
            assert!(!unsupported_endpoint(path, 404));
            assert!(unsupported_endpoint(path, 501));
        }
    }

    #[test]
    fn discovery_stays_on_the_origin_and_recognizes_both_schemas() {
        for version in ["2.0", "2.1"] {
            for url in [
                "https://other.test/nodeinfo",
                "https://m.test.evil/nodeinfo",
                "https://m.test@evil/nodeinfo",
                "http://m.test/nodeinfo",
            ] {
                let d = json!({"links": [{"rel": format!("http://nodeinfo.diaspora.software/ns/schema/{version}"), "href": url}]});
                assert_eq!(nodeinfo_url("https://m.test", &d), None);
            }
            let d = json!({"links": [{"rel": format!("http://nodeinfo.diaspora.software/ns/schema/{version}"), "href": "https://m.test/nodeinfo/2.0"}]});
            assert_eq!(
                nodeinfo_url("https://m.test", &d),
                Some("https://m.test/nodeinfo/2.0")
            );
        }
        assert_eq!(nodeinfo_url("https://m.test", &json!({})), None);
    }

    #[test]
    fn documented_shapes_keep_real_software_separate_from_compatibility_versions() {
        let fixtures: Value =
            serde_json::from_str(include_str!("../../testdata/compat/servers.json")).unwrap();
        for fixture in fixtures.as_array().unwrap() {
            assert_eq!(
                server_label("127.0.0.1", &fixture["instance"], &fixture["nodeinfo"]),
                fixture["label"]
            );
            assert_eq!(
                Limits::from_instance(&fixture["instance"]).max_characters,
                fixture["characters"].as_u64().unwrap() as usize
            );
        }
        assert_eq!(
            server_label("127.0.0.1", &json!({}), &Value::Null),
            "127.0.0.1"
        );
        assert_eq!(
            Limits::from_instance(
                &json!({"configuration": {"statuses": {"max_media_attachments": 0}}})
            )
            .max_media_attachments,
            0
        );
    }

    #[rstest::rstest]
    #[case(include_str!("../../testdata/compat/pleroma-live.json"))]
    #[case(include_str!("../../testdata/compat/akkoma-live.json"))]
    fn live_extension_shapes(#[case] source: &str) {
        use crate::api::{Next, Refusal, next_of, types::Status};
        let fixture: Value = serde_json::from_str(source).unwrap();
        for key in ["instance_v1", "instance_v2"] {
            if fixture[key]["status"] != 200 {
                continue;
            }
            let limits = Limits::from_instance(&fixture[key]["body"]);
            assert_eq!(limits.max_characters, 5000);
            assert_eq!(limits.description_limit, 5000);
            assert_eq!(limits.characters_reserved_per_url, 0);
            assert_eq!(limits.video_size_limit, Some(16_000_000));
            assert_eq!(limits.media_body_limit, Some(16_000_000));
            assert!(limits.extension_quotes);
        }
        let nodeinfo = &fixture["nodeinfo"]["body"];
        assert!(
            server_label("127.0.0.1", &fixture["instance_v1"]["body"], nodeinfo)
                .contains(nodeinfo["software"]["name"].as_str().unwrap())
        );
        let video: Status = serde_json::from_value(fixture["video"]["body"].clone()).unwrap();
        let file = &video.media_attachments[0];
        assert_eq!(file.kind, "video");
        assert_eq!(file.preview_url, file.url);
        let page: Vec<Status> = serde_json::from_value(fixture["page"]["body"].clone()).unwrap();
        assert_eq!(
            next_of(fixture["page"]["link"].as_str()),
            Next::MaxId(page.last().unwrap().id.clone())
        );
        let conversations: Vec<crate::api::types::Conversation> =
            serde_json::from_value(fixture["conversations"]["body"].clone()).unwrap();
        assert_eq!(
            conversations[0].last_status.as_ref().unwrap().visibility,
            "direct"
        );
        for key in [
            "status",
            "blocked",
            "markdown",
            "mfm",
            "emoji_reaction",
            "extension_quote",
        ] {
            let status: Status = serde_json::from_value(fixture[key]["body"].clone()).unwrap();
            assert!(!status.id.is_empty());
            assert!(status.quote_approval.is_none());
            if key == "markdown" || key == "mfm" {
                assert!(
                    crate::html::to_text(&status.content).contains(&format!("compatibility {key}"))
                );
            }
            if key == "extension_quote" {
                let (state, quoted) = status.quote_view().unwrap();
                assert_eq!(state, "accepted");
                assert_eq!(quoted.unwrap().id, fixture["emoji_reaction"]["body"]["id"]);
            }
        }
        for (key, path, code) in [
            ("deleted", "v1/statuses/deleted", 404),
            ("wrong_token", "v1/accounts/verify_credentials", 403),
            ("text_over_limit", "v1/statuses", 422),
            ("upload_over_limit", "v2/media", 413),
        ] {
            assert_eq!(fixture[key]["status"], code);
            let refusal = Refusal {
                status: code,
                body: serde_json::from_value(fixture[key]["body"].clone()).unwrap(),
                reset: None,
            };
            let error = Client::new("http://127.0.0.1:1").refusal(path, refusal);
            assert_eq!(error.kind().exit_code(), 4);
            let expected = fixture[key]["body"]["error"]
                .as_str()
                .map(str::to_string)
                .unwrap_or_else(|| format!("HTTP {code}"));
            assert!(error.message().contains(&expected));
        }
    }
}
