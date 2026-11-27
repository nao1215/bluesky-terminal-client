//! Antennas, lists and channels as tabs of their own: `+` offers the
//! account's antennas, lists and channels, and the one chosen is a tab
//! after the others until the client quits or the account changes.

use super::*;
use crate::i18n::tf;

/// How many feed tabs there may be.
const MAX_FEED_TABS: usize = 13;

/// What a feed tab reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedKind {
    Antenna,
    List,
    Channel,
}

impl FeedKind {
    /// The endpoint the notes come from and the key the ID is sent as.
    pub fn endpoint(self) -> (&'static str, &'static str) {
        match self {
            FeedKind::Antenna => ("antennas/notes", "antennaId"),
            FeedKind::List => ("notes/user-list-timeline", "listId"),
            FeedKind::Channel => ("channels/timeline", "channelId"),
        }
    }

    /// The mark the tab is labeled with.
    pub fn mark(self) -> &'static str {
        match self {
            FeedKind::Antenna => "📡",
            FeedKind::List => "☰",
            FeedKind::Channel => "#",
        }
    }
}

/// An antenna, a list or a channel of a server.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Feed {
    pub kind: FeedKind,
    pub host: String,
    pub id: String,
    pub name: String,
}

/// A feed tab: what it reads, and the notes read.
#[derive(Debug, Clone, Default)]
pub struct FeedTab {
    pub feed: Option<Feed>,
    pub list: List<Note>,
}

impl App {
    /// The tab of feed `i` shown, if the tab is one.
    pub fn feed_tab(&mut self, i: u8) -> Option<&mut FeedTab> {
        self.feeds.get_mut(usize::from(i))
    }

    /// Ask for the first page of feed tab `i`.
    pub(super) fn load_feed(&mut self, i: u8) -> Jobs {
        let Some(feed) = self.feeds.get(usize::from(i)).and_then(|f| f.feed.clone()) else {
            return Vec::new();
        };
        let (seq, job) = self.stamp(Job::FeedNotes { feed, until: None });
        if let Some(t) = self.feed_tab(i) {
            t.list.begin();
            t.list.asked = seq;
        }
        vec![(seq, job)]
    }

    /// The feed tab reading `feed`, if one does.
    fn feed_index(&self, feed: &Feed) -> Option<usize> {
        self.feeds
            .iter()
            .position(|f| f.feed.as_ref() == Some(feed))
    }

    /// Answers for the feed tabs and for `+`'s list.
    pub(super) fn feed_answer(&mut self, seq: u64, event: Event) {
        match event {
            Event::FeedNotes {
                feed,
                until,
                result,
            } => {
                let Some(i) = self.feed_index(&feed) else {
                    return;
                };
                let list = &mut self.feeds[i].list;
                match (until, result) {
                    (None, Ok(notes)) if list.asked == seq => list.set(notes),
                    (None, Err(e)) if list.asked == seq => list.failed(said(&e)),
                    (Some(from), Ok(notes)) => list.append(&from, notes),
                    (Some(from), Err(e)) => {
                        list.more_failed(&from);
                        self.error(said(&e));
                    }
                    _ => {}
                }
            }
            Event::FeedChoices(result) => {
                let Some(Overlay::AddTab { feeds, error, .. }) = &mut self.overlay else {
                    return;
                };
                match result {
                    Ok(list) => *feeds = Some(list),
                    Err(e) => {
                        *feeds = Some(Vec::new());
                        *error = Some(said(&e));
                    }
                }
            }
            _ => {}
        }
    }

    /// What `+` offers: the account's feeds not shown, as they came.
    pub fn tab_choices(&self) -> Vec<Feed> {
        let Some(Overlay::AddTab {
            feeds: Some(feeds), ..
        }) = &self.overlay
        else {
            return Vec::new();
        };
        feeds
            .iter()
            .filter(|f| self.feed_index(f).is_none())
            .cloned()
            .collect()
    }

    /// `+`: the tabs that can be added, the account's feeds read.
    pub(super) fn open_add_tab(&mut self) -> Jobs {
        let feeds_known = self.session.is_some();
        self.overlay = Some(Overlay::AddTab {
            selected: 0,
            feeds: (!feeds_known).then(Vec::new),
            error: None,
        });
        if !feeds_known {
            return Vec::new();
        }
        vec![self.stamp(Job::FeedChoices)]
    }

    pub(super) fn add_tab_key(&mut self, key: KeyEvent) -> Jobs {
        let choices = self.tab_choices();
        let Some(Overlay::AddTab { selected, .. }) = &mut self.overlay else {
            return Vec::new();
        };
        let n = choices.len().max(1);
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => *selected = (*selected + 1) % n,
            KeyCode::Char('k') | KeyCode::Up => *selected = (*selected + n - 1) % n,
            KeyCode::Esc | KeyCode::Char('q' | '+') => self.overlay = None,
            KeyCode::Enter => {
                let Some(chosen) = choices.get(*selected).cloned() else {
                    return Vec::new();
                };
                self.overlay = None;
                return self.add_tab(chosen);
            }
            _ => {}
        }
        Vec::new()
    }

    /// Add `feed` as the last tab and show it.
    fn add_tab(&mut self, feed: Feed) -> Jobs {
        if self.feeds.len() >= MAX_FEED_TABS {
            self.error(tf(
                "at most {} tabs",
                &[&(TABS.len() + MAX_FEED_TABS).to_string()],
            ));
            return Vec::new();
        }
        self.feeds.push(FeedTab {
            feed: Some(feed),
            list: List::default(),
        });
        let tab = Tab::Feed((self.feeds.len() - 1) as u8);
        let label = self.tab_label(tab);
        self.info(tf("tab added: {}", &[&label]));
        self.switch_tab(tab)
    }

    /// The label tab `tab` is shown with.
    pub fn tab_label(&self, tab: Tab) -> String {
        match tab {
            Tab::Feed(i) => match self.feeds.get(usize::from(i)).and_then(|f| f.feed.as_ref()) {
                Some(f) => format!("{} {}", f.kind.mark(), crate::tui::text::one_line(&f.name)),
                None => "?".into(),
            },
            t => crate::i18n::t(t.title()).to_string(),
        }
    }
}
