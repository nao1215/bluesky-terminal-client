//! Search is a draft over any list, then results in the timeline's area.

use super::*;

impl App {
    /// The result list currently shown, including while its box is open.
    pub(crate) fn search_mode(&self) -> Option<SearchMode> {
        (self.tab == Tab::Timeline && self.threads.is_empty())
            .then_some(self.search.active)
            .flatten()
    }

    pub(crate) fn search_query(&self) -> &str {
        match self.search.active {
            Some(SearchMode::Accounts) => &self.search.actors_query,
            _ => &self.search.posts_query,
        }
    }

    pub(super) fn open_search(&mut self) {
        // Reopening keeps the draft and puts the cursor at its end.
        self.search.input = TextInput::single(&self.search.input.text());
        self.overlay = Some(Overlay::Search);
    }

    pub(super) fn search_key(&mut self, key: KeyEvent) -> Vec<Job> {
        match key.code {
            KeyCode::Esc => self.overlay = None,
            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.search.mode = match self.search.mode {
                    SearchMode::Posts => SearchMode::Accounts,
                    SearchMode::Accounts => SearchMode::Posts,
                };
            }
            KeyCode::Enter => return self.run_search(),
            _ => {
                self.search.input.handle_key(key);
            }
        }
        Vec::new()
    }

    pub(super) fn run_search(&mut self) -> Vec<Job> {
        let query = self.search.input.text().trim().to_string();
        if query.is_empty() {
            return Vec::new();
        }
        self.overlay = None;
        self.request_search(self.search.mode, query)
    }

    pub(super) fn refresh_search(&mut self) -> Vec<Job> {
        let Some(mode) = self.search.active else {
            return Vec::new();
        };
        self.request_search(mode, self.search_query().to_string())
    }

    fn request_search(&mut self, mode: SearchMode, query: String) -> Vec<Job> {
        // Keep the displayed list, query and cursor together until success.
        // A failure must leave even the previous mode's content intact.
        self.search.requested = Some((mode, query.clone()));
        self.search.requested_seq = None;
        vec![match mode {
            SearchMode::Posts => Job::SearchPosts(query),
            SearchMode::Accounts => Job::SearchActors(query),
        }]
    }

    pub(super) fn wants_search(&self, mode: SearchMode, query: &str) -> bool {
        self.wants_search_at(mode, query, self.answering)
    }

    pub(super) fn wants_search_at(&self, mode: SearchMode, query: &str, seq: Option<u64>) -> bool {
        self.search
            .requested
            .as_ref()
            .is_some_and(|(m, q)| *m == mode && q == query)
            && self.search.requested_seq == seq
    }

    pub(super) fn show_search(&mut self, mode: SearchMode) {
        self.search.active = Some(mode);
        self.search.requested = None;
        self.search.requested_seq = None;
        self.tab = Tab::Timeline;
        self.threads.clear();
        self.profile.came_from = None;
    }

    pub(super) fn leave_search(&mut self) {
        self.search.active = None;
        self.search.requested = None;
        self.search.requested_seq = None;
    }

    pub(super) fn search_failed(&mut self, error: &Error) {
        self.search.requested = None;
        self.search.requested_seq = None;
        self.fail(error);
        if let Some(status) = &mut self.status {
            status.popup = false;
        }
    }
}
