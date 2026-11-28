//! The columns of the Home tab, as bsky's of its Timeline tab: adding,
//! loading, paging, and keeping them.

use super::*;
use crate::i18n::tf;
use crate::tui::columns::{Columns, Rows, Source, each_list};
use crate::tui::input::TextInput;
use crate::tui::worker::ColumnPage;

impl App {
    /// The Home tab shows the columns when there are any, the home
    /// timeline alone when there are none; the columns not asked for yet
    /// are loaded.
    pub(crate) fn settle_timeline(&mut self) -> Jobs {
        if !matches!(self.tab, Tab::Timeline(TimelineKind::Home) | Tab::Columns) {
            return Vec::new();
        }
        if self.columns.items.is_empty() {
            self.tab = Tab::Timeline(TimelineKind::Home);
            let home = &self.timelines[TimelineKind::Home.index()];
            if !home.loaded && !home.loading {
                return self.load_timeline(TimelineKind::Home);
            }
            return Vec::new();
        }
        self.tab = Tab::Columns;
        let waiting: Vec<u64> = self
            .columns
            .items
            .iter()
            .filter(|c| !c.asked())
            .map(|c| c.id)
            .collect();
        waiting
            .into_iter()
            .flat_map(|id| self.load_column(id))
            .collect()
    }

    /// The name the account in use keeps its columns under.
    fn columns_key(&self) -> Option<String> {
        self.session
            .as_ref()
            .map(|s| format!("{}@{}", s.username, s.host()))
    }

    /// The columns `settings.json` keeps for the account in use, shown on
    /// the Home tab if it is the one shown.
    pub(crate) fn show_columns(&mut self) -> Jobs {
        let sources = self
            .columns_key()
            .and_then(|k| self.settings.columns.get(&k).cloned())
            .unwrap_or_default();
        self.columns = Columns::from_sources(&sources);
        self.settle_timeline()
    }

    /// Keep the columns of the account in use in `settings.json`.
    pub(super) fn save_columns(&mut self) {
        let Some(key) = self.columns_key() else {
            return;
        };
        let sources = self.columns.sources();
        if sources.is_empty() {
            self.settings.columns.remove(&key);
        } else {
            self.settings.columns.insert(key, sources);
        }
        let n = self.columns.items.len();
        let count = n.to_string();
        self.save_settings(if n == 1 {
            tf("{} column", &[&count])
        } else {
            tf("{} columns", &[&count])
        });
    }

    /// Ask for the first page of the column `id`; an answer to an earlier
    /// ask is then of a list loaded again since.
    pub(super) fn load_column(&mut self, id: u64) -> Jobs {
        let Some(source) = self.columns.by_id(id).map(|c| c.source.clone()) else {
            return Vec::new();
        };
        let (seq, job) = self.stamp(Job::Column {
            id,
            source,
            until: None,
        });
        if let Some(c) = self.columns.by_id(id) {
            each_list!(&mut c.rows, |l| {
                l.begin();
                l.asked = seq;
            });
        }
        vec![(seq, job)]
    }

    /// Move in the focused column, and ask for its next page near the end.
    pub(super) fn step_column(&mut self, delta: isize) -> Jobs {
        let Some(c) = self.columns.focused_mut() else {
            return Vec::new();
        };
        let until = each_list!(&mut c.rows, |l| {
            l.step(delta);
            l.want_more()
        });
        let (id, source) = (c.id, c.source.clone());
        until
            .map(|until| {
                self.stamp(Job::Column {
                    id,
                    source,
                    until: Some(until),
                })
            })
            .into_iter()
            .collect()
    }

    /// A page for the column `id`. It lands only in that column, and a first
    /// page only for the column's latest load; a removed column's is dropped.
    pub(super) fn column_page(
        &mut self,
        seq: u64,
        id: u64,
        until: Option<String>,
        result: crate::error::Result<ColumnPage>,
    ) {
        let Some(c) = self.columns.by_id(id) else {
            return;
        };
        let latest = each_list!(&c.rows, |l| l.asked == seq);
        if until.is_none() && !latest {
            return;
        }
        let failed = match (&mut c.rows, &until, result) {
            (Rows::Notes(l), None, Ok(ColumnPage::Notes(page))) => {
                l.set(page);
                None
            }
            (Rows::Notes(l), Some(at), Ok(ColumnPage::Notes(page))) => {
                l.append(at, page);
                None
            }
            (Rows::Notifications(l), None, Ok(ColumnPage::Notifications(page))) => {
                l.set(page);
                None
            }
            (Rows::Notifications(l), Some(at), Ok(ColumnPage::Notifications(page))) => {
                l.append(at, page);
                None
            }
            (rows, _, Err(e)) => {
                let why = said(&e);
                each_list!(rows, |l| match &until {
                    None => l.failed(why.clone()),
                    Some(at) => l.more_failed(at),
                });
                // A first page's failure is said in the column.
                until.is_some().then_some(why)
            }
            _ => None,
        };
        if let Some(why) = failed {
            self.error(why);
        }
    }

    /// What `+` offers: the sources a column can have, the search last.
    pub(crate) fn column_choices(&self) -> Vec<Source> {
        let mut v = vec![Source::Home];
        v.extend(self.column_feeds.iter().cloned());
        v.push(Source::Notifications);
        if let Some(s) = &self.session {
            v.push(Source::Author {
                id: s.user_id.clone(),
                username: s.username.clone(),
            });
        }
        v.push(Source::Search {
            query: String::new(),
        });
        v
    }

    /// `+` on the Home tab: the list of what a column can show. The
    /// account's antennas, lists and channels come into it when they are in.
    pub(super) fn open_add_column(&mut self) -> Jobs {
        if self.session.is_none() {
            let host = self.host().to_string();
            self.error(format!(
                "no account is logged in on {host}: press A then a to log in (or run `msky login --host {host}`)"
            ));
            return Vec::new();
        }
        self.overlay = Some(Overlay::AddColumn {
            selected: 0,
            query: None,
        });
        if self.column_feeds_asked {
            return Vec::new();
        }
        self.column_feeds_asked = true;
        vec![self.stamp(Job::ColumnChoices)]
    }

    /// The account's antennas, lists and channels, for `+` to offer.
    pub(super) fn column_choices_answer(&mut self, result: crate::error::Result<Vec<Source>>) {
        let feeds = match result {
            Ok(feeds) => feeds,
            Err(e) => {
                // Asked again on the next `+`.
                self.column_feeds_asked = false;
                self.error(said(&e));
                return;
            }
        };
        // The + list offers them: its selection stays on its choice.
        let choosing = match &self.overlay {
            Some(Overlay::AddColumn { selected, .. }) => {
                self.column_choices().get(*selected).cloned()
            }
            _ => None,
        };
        self.column_feeds = feeds;
        if let Some(chosen) = choosing {
            let at = self
                .column_choices()
                .iter()
                .position(|c| *c == chosen)
                .unwrap_or(0);
            if let Some(Overlay::AddColumn { selected, .. }) = &mut self.overlay {
                *selected = at;
            }
        }
    }

    /// A key on the list `+` opened, or on the search being typed there.
    pub(super) fn add_column_key(&mut self, key: KeyEvent) -> Jobs {
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
                    return self.add_column(Source::Search { query: q });
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
                if matches!(chosen, Source::Search { .. }) {
                    *query = Some(TextInput::single(""));
                    return Vec::new();
                }
                return self.add_column(chosen);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Add a column of `source`, focus it, keep it, and load it. The first
    /// one added comes beside the home timeline, which becomes a column of
    /// its own.
    pub(super) fn add_column(&mut self, source: Source) -> Jobs {
        self.overlay = None;
        if self.columns.items.is_empty() && source != Source::Home {
            self.columns.add(Source::Home);
        }
        self.columns.add(source);
        self.save_columns();
        self.tab = Tab::Columns;
        self.settle_timeline()
    }

    /// `x` on the Home tab with columns: remove the focused one once `y`
    /// confirms.
    pub(super) fn ask_remove_column(&mut self) {
        if let Some(c) = self.columns.focused() {
            let (id, title) = (c.id, c.source.title());
            self.info(tf(
                "press y to remove the column {}, any other key to keep it",
                &[&title],
            ));
            self.confirm_remove_column = Some(id);
        }
    }

    /// The key after `x`.
    pub(super) fn answer_remove_column(&mut self, id: u64, key: KeyEvent) -> Jobs {
        if key.code == KeyCode::Char('y') && self.columns.focused().is_some_and(|c| c.id == id) {
            self.columns.remove_focused();
            self.save_columns();
            // The last one gone, the home timeline is shown alone again.
            return self.settle_timeline();
        }
        self.info(n!("the column stays"));
        Vec::new()
    }

    /// The note the focused column's selection is on.
    pub(super) fn column_note(&self) -> Option<&Note> {
        match &self.columns.focused()?.rows {
            Rows::Notes(l) => l.current(),
            Rows::Notifications(l) => l.current().and_then(|n| n.note.as_ref()),
        }
    }

    /// R on the Home tab with columns: the focused column read again.
    pub(super) fn refresh_column(&mut self) -> Jobs {
        match self.columns.focused().map(|c| c.id) {
            Some(id) => self.load_column(id),
            None => Vec::new(),
        }
    }
}
