//! Column selection, persistence and independent requests.
use super::*;
use crate::tui::columns::{Deck, Source};
use crate::tui::worker::ColumnPage;

impl App {
    pub(crate) fn in_deck(&self) -> bool {
        self.tab == Tab::Home && self.threads.is_empty() && !self.deck.columns.is_empty()
    }
    fn deck_key(&self) -> Option<String> {
        self.session
            .as_ref()
            .map(|s| format!("{}@{}", s.username, s.service))
    }
    pub(super) fn save_deck(&mut self) {
        let Some(key) = self.deck_key() else { return };
        if let Some(settings) = &mut self.settings {
            settings.decks.insert(
                key,
                self.deck.columns.iter().map(|c| c.source.clone()).collect(),
            );
            self.settings_save = true;
        }
    }
    pub(super) fn restore_deck(&mut self) {
        self.deck = Deck::default();
        if let Some(sources) = self
            .deck_key()
            .and_then(|key| self.settings.as_ref()?.decks.get(&key))
            .cloned()
        {
            for source in sources {
                self.deck.add(source);
            }
            self.deck.focus = 0;
        }
    }
    pub(crate) fn load_deck(&mut self) -> Jobs {
        let ids: Vec<_> = self
            .deck
            .columns
            .iter()
            .filter(|c| {
                if c.source == Source::Notifications {
                    !c.notifications.loaded && c.notifications.waiting.is_none()
                } else {
                    !c.posts.loaded && c.posts.waiting.is_none()
                }
            })
            .map(|c| c.id)
            .collect();
        ids.into_iter()
            .flat_map(|id| self.load_column(id, false))
            .collect()
    }
    pub(super) fn load_column(&mut self, id: u64, more: bool) -> Jobs {
        let Some(c) = self.deck.columns.iter().find(|c| c.id == id) else {
            return Vec::new();
        };
        let next = if more {
            if c.source == Source::Notifications {
                c.notifications.next.clone()
            } else {
                c.posts.next.clone()
            }
        } else {
            None
        };
        let job = self.job(Job::Column {
            id,
            source: c.source.clone(),
            next,
        });
        let Some(c) = self.deck.columns.iter_mut().find(|c| c.id == id) else {
            return Vec::new();
        };
        if c.source == Source::Notifications {
            c.notifications.waiting = Some(job.0);
        } else {
            c.posts.waiting = Some(job.0);
        }
        vec![job]
    }
    pub(super) fn step_column(&mut self, by: isize) -> Jobs {
        let Some(c) = self.deck.columns.get_mut(self.deck.focus) else {
            return Vec::new();
        };
        let wants = if c.source == Source::Notifications {
            c.notifications.step(by);
            c.notifications.wants_more()
        } else {
            c.posts.step(by);
            c.posts.wants_more()
        };
        let id = c.id;
        if wants {
            self.load_column(id, true)
        } else {
            Vec::new()
        }
    }
    pub(super) fn add_column(&mut self, source: Source) -> Jobs {
        if self.deck.columns.is_empty() && source != Source::Home {
            self.deck.add(Source::Home);
            self.deck.columns[0].posts = self.timelines[0].clone();
            self.deck.columns[0].posts.waiting = None;
        }
        self.deck.add(source);
        self.overlay = None;
        self.save_deck();
        self.load_deck()
    }
    pub(super) fn open_columns(&mut self) -> Jobs {
        if self.session.is_none() {
            self.needs_account();
            return Vec::new();
        }
        self.overlay = Some(Overlay::AddColumn {
            selected: 0,
            query: None,
        });
        if self.deck.choices_waiting.is_some() {
            return Vec::new();
        }
        let job = self.job(Job::ColumnChoices);
        self.deck.choices_waiting = Some(job.0);
        vec![job]
    }
    pub(crate) fn column_choices(&self) -> Vec<Source> {
        let mut choices = vec![
            Source::Home,
            Source::Notifications,
            Source::Tag(String::new()),
            Source::Search(String::new()),
        ];
        choices.extend(self.deck.choices.clone());
        choices
    }
    pub(super) fn column_key(
        &mut self,
        mut selected: usize,
        mut query: Option<TextInput>,
        key: KeyEvent,
    ) -> Jobs {
        let choices = self.column_choices();
        let source = choices.get(selected).cloned().unwrap_or(Source::Home);
        if let Some(input) = &mut query {
            match key.code {
                KeyCode::Esc => query = None,
                KeyCode::Enter => {
                    let text = input.text().trim().to_string();
                    let text = if matches!(source, Source::Tag(_)) {
                        text.trim_start_matches('#').to_string()
                    } else {
                        text
                    };
                    if !text.is_empty() {
                        return self.add_column(if matches!(source, Source::Tag(_)) {
                            Source::Tag(text)
                        } else {
                            Source::Search(text)
                        });
                    }
                }
                _ => {
                    input.handle_key(key);
                }
            }
        } else {
            match key.code {
                KeyCode::Esc | KeyCode::Char('q' | '+') => return Vec::new(),
                KeyCode::Char('j') | KeyCode::Down => selected = (selected + 1) % choices.len(),
                KeyCode::Char('k') | KeyCode::Up => {
                    selected = (selected + choices.len() - 1) % choices.len()
                }
                KeyCode::Enter => {
                    if matches!(&source, Source::Tag(q) | Source::Search(q) if q.is_empty()) {
                        query = Some(TextInput::single(""));
                    } else {
                        return self.add_column(source);
                    }
                }
                _ => {}
            }
        }
        self.overlay = Some(Overlay::AddColumn { selected, query });
        Vec::new()
    }
    pub(super) fn column_answer(
        &mut self,
        seq: u64,
        id: u64,
        first: bool,
        result: Result<ColumnPage>,
    ) {
        let Some(c) = self.deck.columns.iter_mut().find(|c| c.id == id) else {
            return;
        };
        match result {
            Ok(ColumnPage::Posts(p)) => {
                c.posts.take(seq, first, Ok(p));
            }
            Ok(ColumnPage::Notifications(p)) => {
                let next = next_of(&p, |n: &Notification| &n.id);
                c.notifications.take(seq, first, Ok((p.items, next)));
            }
            Err(e) => {
                if c.source == Source::Notifications {
                    c.notifications.take(seq, first, Err(e));
                } else {
                    c.posts.take(seq, first, Err(e));
                }
            }
        }
    }
}
