//! Lists and notification markers shared by the commands and full-screen client.
use super::*;

#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct NotificationMarker {
    pub last_read_id: String,
}

impl Client {
    pub(crate) fn account_list(
        &self,
        path: &str,
        limit: usize,
        max_id: Option<&str>,
    ) -> Result<Page<Account>> {
        let mut query = vec![("limit", limit.min(MAX_PAGE).to_string())];
        if let Some(id) = max_id {
            query.push(("max_id", id.to_string()));
        }
        self.get_list(path, &query)
    }

    pub(crate) fn notification_marker(&self) -> Result<Option<NotificationMarker>> {
        #[derive(Deserialize)]
        struct Markers {
            notifications: Option<NotificationMarker>,
        }
        serde_json::from_value::<Markers>(
            self.get_one("v1/markers", &[("timeline[]", "notifications".into())])?,
        )
        .map(|m| m.notifications)
        .map_err(|e| Error::api(format!("invalid notification marker: {e}")))
    }

    pub(crate) fn unread_notifications(&self) -> Result<usize> {
        #[derive(Deserialize)]
        struct Count {
            count: usize,
        }
        serde_json::from_value::<Count>(self.get_one("v1/notifications/unread_count", &[])?)
            .map(|c| c.count)
            .map_err(|e| Error::api(format!("invalid unread count: {e}")))
    }

    pub(crate) fn mark_notifications(&self, id: &str) -> Result<()> {
        self.post(
            "v1/markers",
            &json!({"notifications": {"last_read_id": id}}),
            None,
        )
        .map(drop)
    }

    pub(crate) fn categorized_report(
        &self,
        account: &str,
        status: Option<&str>,
        comment: &str,
        category: &str,
    ) -> Result<()> {
        let mut body = json!({"account_id": account, "comment": comment, "forward": false, "category": category});
        if let Some(id) = status {
            body["status_ids"] = json!([id]);
        }
        self.post("v1/reports", &body, None).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::stub_tests::{reply, serve};
    #[test]
    fn notification_api_reads_marker_and_count_and_writes_the_exact_nested_body() {
        let (origin, calls) = serve(Arc::new(|c, _| match (c.method.as_str(), c.path()) {
            ("GET", "/api/v1/markers") => reply(200, r#"{"notifications":{"last_read_id":"9"}}"#),
            ("GET", "/api/v1/notifications/unread_count") => reply(200, r#"{"count":42}"#),
            _ => reply(200, "{}"),
        }));
        let client = Client::new(&origin).with_token(Some("test-only".into()));
        assert_eq!(
            client.notification_marker().unwrap().unwrap().last_read_id,
            "9"
        );
        assert_eq!(client.unread_notifications().unwrap(), 42);
        client.mark_notifications("10").unwrap();
        let calls = calls.lock().unwrap();
        assert_eq!(
            calls[0].query("timeline%5B%5D").as_deref(),
            Some("notifications")
        );
        assert_eq!(calls[2].method, "POST");
        assert_eq!(
            serde_json::from_str::<Value>(&calls[2].body).unwrap(),
            json!({"notifications":{"last_read_id":"10"}})
        );
    }
    #[test]
    fn accepted_quote_populates_the_common_quote_id() {
        let status: Status = serde_json::from_value(
            json!({"id":"43","quote":{"state":"accepted","quoted_status":{"id":"42"}}}),
        )
        .unwrap();
        assert_eq!(
            crate::cli::format::status_json(&status, "https://example.test")["quoteId"],
            "42"
        );
    }
}
