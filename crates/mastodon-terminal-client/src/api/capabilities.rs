//! Endpoint evidence, with optional public server metadata for diagnostics.
//! Probe features first; only an unambiguous Mastodon release can explain an
//! optional route missing on an older server.

use super::*;

/// A 404 on a resource can mean absent, private or deleted. Only collection
/// routes with no resource lookup are unambiguous enough to classify here.
fn resource_free_endpoint(path: &str) -> bool {
    matches!(
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
    )
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
            let status = self.status(id)?;
            if status.id.is_empty() {
                return Err(Error::api("invalid status while checking quote support"));
            }
            if status.quote_approval.is_some() {
                true
            } else if self.limits().extension_quotes {
                return Ok("quote_id");
            } else {
                false
            }
        } else {
            let account = self.get_one("v1/accounts/verify_credentials", &[])?;
            if account["id"].as_str().is_none_or(str::is_empty) {
                return Err(Error::api("invalid account while checking quote support"));
            }
            account["source"]["quote_policy"].is_string()
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
        self.unsupported_with_instance(feature, &self.public_instance())
    }

    fn public_instance(&self) -> Value {
        let url = format!("{}/api/v2/instance", self.origin);
        match self.exchange("server metadata", false, &|| self.agent.get(&url).call()) {
            Ok(Answer::Took { value, .. }) => value,
            Ok(Answer::Refused(r)) if r.status == 404 => self
                .public_document(&format!("{}/api/v1/instance", self.origin))
                .unwrap_or(Value::Null),
            _ => Value::Null,
        }
    }

    fn public_nodeinfo(&self) -> Value {
        let discovery = self
            .public_document(&format!("{}/.well-known/nodeinfo", self.origin))
            .unwrap_or(Value::Null);
        nodeinfo_url(&self.origin, &discovery)
            .and_then(|url| self.public_document(url))
            .unwrap_or(Value::Null)
    }

    fn unsupported_with_instance(&self, feature: &str, instance: &Value) -> Error {
        Error::unsupported(
            feature,
            &server_label(self.host(), instance, &self.public_nodeinfo()),
        )
    }

    /// Fall back only when the v2 instance route is absent. Authentication and
    /// transport failures must not become evidence of an old server.
    pub(super) fn instance(&self) -> Result<Value> {
        let path = "v2/instance";
        let url = format!("{}/api/{path}", self.origin);
        match self.exchange(path, true, &|| self.signed(self.agent.get(&url)).call())? {
            Answer::Took { value, .. } => Ok(value),
            Answer::Refused(r) if r.status == 404 => self.get_one("v1/instance", &[]),
            Answer::Refused(r) => Err(self.refusal(path, r)),
        }
    }

    /// A 501 explicitly refuses a feature. For optional routes that can also
    /// hide resources, use an old Mastodon release only after the failed probe.
    /// A compatibility version is not evidence about another implementation.
    pub(super) fn capability_refusal(&self, path: &str, status: u16) -> Option<Error> {
        if status == 501 {
            return Some(self.unsupported(path));
        }
        if status != 404 {
            return None;
        }
        if let Some(minimum) = minimum_release(path) {
            let instance = self.public_instance();
            let nodeinfo = self.public_nodeinfo();
            let compatible = compatible_version(&instance)
                || nodeinfo["software"]["name"]
                    .as_str()
                    .is_some_and(|name| name != "mastodon");
            let absent = if compatible {
                resource_free_endpoint(path)
            } else {
                mastodon_release(&instance).is_some_and(|v| v < minimum)
            };
            return absent.then(|| {
                Error::unsupported(path, &server_label(self.host(), &instance, &nodeinfo))
            });
        }
        resource_free_endpoint(path).then(|| self.unsupported(path))
    }
}

fn compatible_version(instance: &Value) -> bool {
    instance["version"]
        .as_str()
        .is_some_and(|v| v.to_ascii_lowercase().contains("compatible"))
}

/// Only interpret a Mastodon release, never the compatibility version of a fork.
fn mastodon_release(instance: &Value) -> Option<(u32, u32)> {
    let mut parts = instance["version"].as_str()?.split('.');
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

/// Introduction versions from docs.joinmastodon.org/methods/{endpoint}/.
fn minimum_release(path: &str) -> Option<(u32, u32)> {
    match path {
        "v1/bookmarks" => Some((3, 1)),
        "v1/markers" => Some((3, 0)),
        "v1/notifications/unread_count" => Some((4, 3)),
        "v1/lists" => Some((2, 1)),
        "v1/conversations" => Some((2, 6)),
        _ if path.starts_with("v1/statuses/")
            && (path.ends_with("/bookmark") || path.ends_with("/unbookmark")) =>
        {
            Some((3, 1))
        }
        _ if path.starts_with("v1/conversations/") => Some((2, 6)),
        _ if path.starts_with("v1/lists/") || path.starts_with("v1/timelines/list/") => {
            Some((2, 1))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
