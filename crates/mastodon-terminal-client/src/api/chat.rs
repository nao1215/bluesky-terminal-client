//! Mastodon conversations are threads containing direct statuses.
use super::*;
impl Client {
    pub(crate) fn conversations(&self, next: Option<&str>) -> Result<Page<types::Conversation>> {
        let mut query = vec![("limit", "30".into())];
        if let Some(next) = next {
            query.push(("max_id", next.into()));
        }
        self.get_list("v1/conversations", &query)
    }
    pub(crate) fn read_conversation(&self, id: &str) -> Result<()> {
        self.post(
            &format!("v1/conversations/{}/read", path_part(id)),
            &json!({}),
            None,
        )
        .map(|_| ())
    }
}
