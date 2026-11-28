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

/// Context can contain direct branches addressed to different participants.
/// Those branches are separate conversations, even though they share a root.
pub(crate) fn same_conversation(
    status: &Status,
    accounts: &[Account],
    me: Option<(&str, &str)>,
) -> bool {
    if status.visibility != "direct" {
        return false;
    }
    let other = |id: &str, acct: &str| {
        !acct.is_empty()
            && !me.is_some_and(|(me_id, me_acct)| {
                (!id.is_empty() && id == me_id) || acct.eq_ignore_ascii_case(me_acct)
            })
    };
    let expected: std::collections::HashSet<_> = accounts
        .iter()
        .filter(|a| other(&a.id, &a.acct))
        .map(|a| a.acct.to_ascii_lowercase())
        .collect();
    let actual: std::collections::HashSet<_> =
        std::iter::once((status.account.id.as_str(), status.account.acct.as_str()))
            .chain(
                status
                    .mentions
                    .iter()
                    .map(|m| (m.id.as_str(), m.acct.as_str())),
            )
            .filter(|(id, acct)| other(id, acct))
            .map(|(_, acct)| acct.to_ascii_lowercase())
            .collect();
    expected == actual
}
