//! Antennas, lists and channels as tabs of their own, as bsky's columns
//! of feeds: `+` offers the tabs not shown and the account's antennas,
//! lists and channels, `x` closes the tab shown (`y` confirms). The `tabs`
//! setting keeps each as `kind/host/id/name`; one of another server is
//! kept there but shown only on its server.

use super::*;
use crate::i18n::{n, tf};

/// What a feed tab reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FeedKind {
    Antenna,
    List,
    Channel,
}

impl FeedKind {
    pub const ALL: [FeedKind; 3] = [FeedKind::Antenna, FeedKind::List, FeedKind::Channel];

    /// The name the `tabs` setting gives the kind.
    pub fn name(self) -> &'static str {
        match self {
            FeedKind::Antenna => "antenna",
            FeedKind::List => "list",
            FeedKind::Channel => "channel",
        }
    }

    fn from_name(s: &str) -> Option<FeedKind> {
        FeedKind::ALL.into_iter().find(|k| k.name() == s)
    }

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

impl Feed {
    /// As the `tabs` setting keeps it: `kind/host/id/name`.
    pub fn setting(&self) -> String {
        // A comma would end the name where the settings screen lists the
        // tabs.
        let name = self.name.replace(',', " ");
        format!("{}/{}/{}/{name}", self.kind.name(), self.host, self.id)
    }

    /// One the `tabs` setting names, if `s` is one.
    pub fn parse(s: &str) -> Option<Feed> {
        let mut parts = s.trim().splitn(4, '/');
        let kind = FeedKind::from_name(parts.next()?)?;
        let host = parts.next().filter(|h| !h.is_empty())?.to_ascii_lowercase();
        let id = parts.next().filter(|i| !i.is_empty())?.to_string();
        let name = parts.next().unwrap_or_default().to_string();
        Some(Feed {
            kind,
            host,
            id,
            name,
        })
    }

    /// Whether `s`, a name in the `tabs` setting, looks like a feed's.
    pub fn named(s: &str) -> bool {
        s.split('/').next().and_then(FeedKind::from_name).is_some()
    }
}

/// A feed tab: what it reads, and the notes read.
#[derive(Debug, Clone, Default)]
pub struct FeedTab {
    pub feed: Option<Feed>,
    pub list: List<Note>,
}

/// What `+` offers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabChoice {
    Builtin(Tab),
    Feed(Feed),
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

    /// What `+` offers: the tabs not shown, then the account's feeds not
    /// shown, as they came.
    pub fn tab_choices(&self) -> Vec<TabChoice> {
        let mut v: Vec<TabChoice> = ALL_TABS
            .iter()
            .filter(|t| !self.tabs.contains(t))
            .map(|t| TabChoice::Builtin(*t))
            .collect();
        if let Some(Overlay::AddTab {
            feeds: Some(feeds), ..
        }) = &self.overlay
        {
            v.extend(
                feeds
                    .iter()
                    .filter(|f| {
                        self.feed_index(f)
                            .is_none_or(|i| !self.tabs.contains(&Tab::Feed(i as u8)))
                    })
                    .cloned()
                    .map(TabChoice::Feed),
            );
        }
        v
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

    /// Add `choice` as the last tab, show it and keep the tabs.
    fn add_tab(&mut self, choice: TabChoice) -> Jobs {
        let tab = match choice {
            TabChoice::Builtin(t) => t,
            TabChoice::Feed(feed) => {
                let i = match self.feed_index(&feed) {
                    Some(i) => i,
                    None => {
                        self.feeds.push(FeedTab {
                            feed: Some(feed),
                            list: List::default(),
                        });
                        self.feeds.len() - 1
                    }
                };
                Tab::Feed(i as u8)
            }
        };
        if self.tabs.len() >= MAX_TABS {
            self.error(tf("at most {} tabs", &[&MAX_TABS.to_string()]));
            return Vec::new();
        }
        self.tabs.push(tab);
        let label = self.tab_label(tab);
        self.keep_tabs(tf("tab added: {}", &[&label]));
        self.switch_tab(tab)
    }

    /// `x`: close the tab shown, once `y` confirms; the last one stays.
    pub(super) fn ask_close_tab(&mut self) {
        if self.tabs.len() <= 1 {
            self.error(n!("the last tab stays"));
            return;
        }
        let label = self.tab_label(self.tab);
        self.info(tf(
            "press y to close the {} tab, any other key to keep it",
            &[&label],
        ));
        self.confirm_close_tab = Some(self.tab);
    }

    /// The key after `x`.
    pub(super) fn answer_close_tab(&mut self, tab: Tab, key: KeyEvent) -> Jobs {
        self.confirm_close_tab = None;
        if key.code != KeyCode::Char('y') || !self.tabs.contains(&tab) || self.tabs.len() <= 1 {
            self.info(n!("kept"));
            return Vec::new();
        }
        let at = self.tabs.iter().position(|t| *t == tab).unwrap_or(0);
        self.tabs.retain(|t| *t != tab);
        let label = self.tab_label(tab);
        self.keep_tabs(tf("tab closed: {}", &[&label]));
        let next = self.tabs[at.min(self.tabs.len() - 1)];
        self.switch_tab(next)
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

    /// The `tabs` setting for the tabs now: the feed tabs of other
    /// servers, which this one does not show, are kept where they were.
    pub(super) fn tabs_setting(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .tabs
            .iter()
            .map(|t| match t {
                Tab::Feed(i) => self
                    .feeds
                    .get(usize::from(*i))
                    .and_then(|f| f.feed.as_ref())
                    .map(Feed::setting)
                    .unwrap_or_default(),
                t => t.setting_name().to_string(),
            })
            .filter(|s| !s.is_empty())
            .collect();
        let host = self.host().to_string();
        for s in self.settings.tabs.iter().flatten() {
            if let Some(f) = Feed::parse(s)
                && f.host != host
                && !out.contains(s)
            {
                out.push(s.clone());
            }
        }
        out
    }

    /// Keep the tabs as they are now in the settings.
    pub(super) fn keep_tabs(&mut self, note: String) {
        self.settings.tabs = Some(self.tabs_setting());
        self.save_settings(note);
    }

    /// The tabs the setting names, for this server: built in, and the feed
    /// tabs of this server, in the order named. A name that is neither is
    /// said; a list naming no tab at all gives the default ones.
    pub fn tabs_for_host(&mut self, names: &[String]) -> (Vec<Tab>, Option<String>) {
        let host = self.host().to_ascii_lowercase();
        let mut feeds: Vec<FeedTab> = Vec::new();
        let mut tabs: Vec<Tab> = Vec::new();
        let mut unknown = Vec::new();
        for n in names {
            let tab = if Feed::named(n) {
                match Feed::parse(n) {
                    Some(f) if f.host == host => {
                        let i = match feeds.iter().position(|t| t.feed.as_ref() == Some(&f)) {
                            Some(i) => i,
                            None => {
                                feeds.push(FeedTab {
                                    feed: Some(f),
                                    list: List::default(),
                                });
                                feeds.len() - 1
                            }
                        };
                        Some(Tab::Feed(i as u8))
                    }
                    // Another server's: kept in the setting, not shown here.
                    Some(_) => None,
                    None => {
                        unknown.push(n.clone());
                        None
                    }
                }
            } else {
                let t = Tab::from_name(n);
                if t.is_none() {
                    unknown.push(n.clone());
                }
                t
            };
            if let Some(t) = tab
                && !tabs.contains(&t)
                && tabs.len() < MAX_TABS
            {
                tabs.push(t);
            }
        }
        if tabs.is_empty() {
            tabs = DEFAULT_TABS.to_vec();
        }
        let warning = (!unknown.is_empty()).then(|| {
            tf(
                "settings.json: {} is not a tab (home, local, social, global, notifications, search, profile, chat, or antenna/, list/, channel/ as + adds them)",
                &[&unknown.join(", ")],
            )
        });
        self.feeds = feeds;
        (tabs, warning)
    }
}
