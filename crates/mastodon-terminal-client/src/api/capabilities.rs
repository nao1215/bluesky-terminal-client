//! Capability failures use the shared command error, without treating a missing
//! resource or an authentication refusal as an unsupported server feature.
use super::*;

impl Client {
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

    /// Unknown parameters may be silently ignored by older Mastodon releases.
    /// Check quote support before any media upload or status write, in both UIs.
    pub(crate) fn posting_limits(&self, quote: bool) -> Result<Limits> {
        if !quote {
            return Ok(self.limits());
        }
        let instance = self.instance()?;
        let limits = Limits::from_instance(&instance);
        if limits.quotes {
            return Ok(limits);
        }
        if types::release(&instance).is_none()
            && instance["api_versions"]["mastodon"].as_u64().is_none()
        {
            return Err(Error::api(
                "cannot determine whether this server supports quote posts",
            ));
        }
        Err(Error::new(
            Kind::Unsupported,
            "this server does not support quote posts (Mastodon 4.5 or API version 7 is required)",
        ))
    }

    pub(super) fn unsupported_endpoint(&self, path: &str, status: u16) -> bool {
        if status == 501 {
            return true;
        }
        if status != 404 {
            return false;
        }
        let Some(minimum) = minimum_release(path) else {
            return false;
        };
        // A 404 alone also means a private/missing object or a proxy refusal.
        // Only a known older release establishes that the route is absent.
        self.instance()
            .ok()
            .and_then(|v| types::release(&v))
            .is_some_and(|v| v < minimum)
    }
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
