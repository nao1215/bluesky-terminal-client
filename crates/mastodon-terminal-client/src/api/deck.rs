//! Account lists and followed hashtags offered as columns.
use super::*;
#[derive(Debug, Deserialize)]
pub(crate) struct AccountList {
    pub id: String,
    pub title: String,
}
impl Client {
    pub(crate) fn lists(&self) -> Result<Vec<AccountList>> {
        self.get_list("v1/lists", &[]).map(|p| p.items)
    }
    pub(crate) fn followed_tags(&self) -> Result<Vec<types::Tag>> {
        let mut tags = Vec::new();
        let mut query = vec![("limit", "100".into())];
        let mut seen = std::collections::HashSet::new();
        loop {
            let page = self.get_list("v1/followed_tags", &query)?;
            tags.extend(page.items);
            let Next::MaxId(id) = page.next else { break };
            if !seen.insert(id.clone()) {
                break;
            }
            query = vec![("limit", "100".into()), ("max_id", id)];
        }
        Ok(tags)
    }
}
