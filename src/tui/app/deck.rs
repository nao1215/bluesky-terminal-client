//! The Columns tab: adding, loading, paging, and keeping its columns.

use super::*;

impl App {
    /// The columns `settings.json` keeps for the account in use.
    pub(super) fn load_columns(&mut self) {
        let did = self
            .session
            .as_ref()
            .map(|s| s.did.clone())
            .unwrap_or_default();
        let sources = self.settings.columns.get(&did).cloned().unwrap_or_default();
        self.columns = Columns::from_sources(&sources);
    }

    /// Keep the columns of the account in use in `settings.json`.
    pub(super) fn save_columns(&mut self) {
        let Some(did) = self.session.as_ref().map(|s| s.did.clone()) else {
            return;
        };
        let sources = self.columns.sources();
        if sources.is_empty() {
            self.settings.columns.remove(&did);
        } else {
            self.settings.columns.insert(did, sources);
        }
        let n = self.columns.items.len();
        self.save_settings(format!("{n} column{}", if n == 1 { "" } else { "s" }));
    }

    /// Ask for the first page of the column `id`, dropping any answer to an
    /// earlier ask.
    pub(super) fn load_column(&mut self, id: u64) -> Vec<Job> {
        let Some(c) = self.columns.by_id(id) else {
            return Vec::new();
        };
        c.generation += 1;
        match &mut c.rows {
            Rows::Posts(l) => l.begin(),
            Rows::Notifications(l) => l.begin(),
        }
        vec![Job::Column {
            id,
            generation: c.generation,
            feed: columns::feed_of(&c.source),
            cursor: None,
        }]
    }

    /// Move in the focused column, and ask for its next page near the end.
    pub(super) fn step_column(&mut self, delta: isize) -> Vec<Job> {
        let Some(c) = self.columns.focused_mut() else {
            return Vec::new();
        };
        let cursor = match &mut c.rows {
            Rows::Posts(l) => {
                l.step(delta);
                l.want_more()
            }
            Rows::Notifications(l) => {
                l.step(delta);
                l.want_more()
            }
        };
        cursor
            .map(|cursor| Job::Column {
                id: c.id,
                generation: c.generation,
                feed: columns::feed_of(&c.source),
                cursor: Some(cursor),
            })
            .into_iter()
            .collect()
    }

    /// A page for the column `id`. It lands only in that column, and a first
    /// page only for the column's latest load; a removed column's is dropped.
    pub(super) fn column_page(
        &mut self,
        id: u64,
        generation: u64,
        cursor: Option<String>,
        result: crate::error::Result<MorePage>,
    ) {
        let Some(c) = self.columns.by_id(id) else {
            return;
        };
        if cursor.is_none() && generation != c.generation {
            return;
        }
        let failed = match (&mut c.rows, &cursor, result) {
            (Rows::Posts(l), None, Ok(MorePage::Posts(page))) => {
                l.set(page);
                None
            }
            (Rows::Posts(l), Some(at), Ok(MorePage::Posts(page))) => {
                l.append(at, page);
                None
            }
            (Rows::Notifications(l), None, Ok(MorePage::Notifications(page))) => {
                l.set(page);
                None
            }
            (Rows::Notifications(l), Some(at), Ok(MorePage::Notifications(page))) => {
                l.append(at, page);
                None
            }
            (rows, _, Err(e)) => {
                match (rows, &cursor) {
                    (Rows::Posts(l), None) => l.failed(&e),
                    (Rows::Notifications(l), None) => l.failed(&e),
                    (Rows::Posts(l), Some(_)) => l.more_pending = false,
                    (Rows::Notifications(l), Some(_)) => l.more_pending = false,
                }
                Some(e)
            }
            _ => None,
        };
        if let Some(e) = failed {
            self.fail(&e);
        }
    }

    /// What `+` offers: the sources a column can have, the search last.
    pub fn column_choices(&self) -> Vec<columns::Source> {
        let mut v = vec![columns::Source::Following];
        v.extend(self.feeds.iter().map(|f| columns::Source::Feed {
            uri: f.info.uri.clone(),
            name: f.info.name.clone(),
        }));
        v.push(columns::Source::Notifications);
        if let Some(s) = &self.session {
            v.push(columns::Source::Author {
                did: s.did.clone(),
                handle: s.handle.clone(),
            });
        }
        v.push(columns::Source::Search {
            query: String::new(),
        });
        v
    }

    /// A key on the list `+` opened, or on the search being typed there.
    pub(super) fn add_column_key(&mut self, key: KeyEvent) -> Vec<Job> {
        let choices = self.column_choices();
        let Some(Overlay::AddColumn { selected, query }) = &mut self.overlay else {
            return Vec::new();
        };
        if let Some(input) = query {
            match key.code {
                KeyCode::Esc => *query = None,
                KeyCode::Enter => {
                    let q = input.text().trim().to_string();
                    if q.is_empty() {
                        return Vec::new();
                    }
                    return self.add_column(columns::Source::Search { query: q });
                }
                _ => {
                    input.handle_key(key);
                }
            }
            return Vec::new();
        }
        let n = choices.len();
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => *selected = (*selected + 1) % n,
            KeyCode::Char('k') | KeyCode::Up => *selected = (*selected + n - 1) % n,
            KeyCode::Esc | KeyCode::Char('q' | '+') => self.overlay = None,
            KeyCode::Enter => {
                let chosen = choices[(*selected).min(n - 1)].clone();
                if matches!(chosen, columns::Source::Search { .. }) {
                    *query = Some(TextInput::single(""));
                    return Vec::new();
                }
                return self.add_column(chosen);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Add a column of `source`, focus it, keep it, and load it.
    pub(super) fn add_column(&mut self, source: columns::Source) -> Vec<Job> {
        self.overlay = None;
        let id = self.columns.add(source);
        self.save_columns();
        self.load_column(id)
    }
}
