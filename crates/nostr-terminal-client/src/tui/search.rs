//! Search replaces the visible list while retaining its exact return state.
use super::{
    app::{App, List, Tab},
    worker::Job,
};
use crate::api::event::Event;

#[derive(Default)]
pub(super) struct Search {
    pub query: String,
    pub list: List,
    pub reset_selection: bool,
    return_to: Option<ReturnTo>,
}

struct ReturnTo {
    tab: Tab,
    column: Option<String>,
    list: List,
    threads: Vec<(Event, List)>,
    profile: String,
    author: Option<String>,
    relationship: Option<(String, bool, bool)>,
}

impl App {
    pub(super) fn searching(&self) -> bool {
        self.search.return_to.is_some() && self.tab == Tab::Home && self.extra.column.is_none()
    }

    pub(super) fn start_search(&mut self, query: String) -> Vec<Job> {
        // Keep the previous content until a successful answer replaces it.
        let previous = self.list().clone();
        if self.search.return_to.is_none() {
            let column = (self.tab == Tab::Home)
                .then_some(self.extra.column)
                .flatten();
            let list = column
                .and_then(|i| self.extra.column_lists.get(i))
                .unwrap_or(&self.lists[self.tab.index()])
                .clone();
            self.search.return_to = Some(ReturnTo {
                tab: self.tab,
                column: column.and_then(|i| self.extra.columns.get(i).cloned()),
                list,
                threads: std::mem::take(&mut self.threads),
                profile: self.profile.clone(),
                author: self.extra.author.clone(),
                relationship: self.extra.relationship.clone(),
            });
        }
        self.search.query = query;
        self.search.reset_selection = true;
        self.search.list = List {
            loading: None,
            ..previous
        };
        self.tab = Tab::Home;
        self.extra.column = None;
        self.extra.author = None;
        self.extra.relationship = None;
        self.threads.clear();
        self.load(true)
    }

    pub(super) fn leave_search(&mut self) {
        let Some(saved) = self.search.return_to.take() else {
            return;
        };
        self.tab = saved.tab;
        self.extra.column = saved
            .column
            .and_then(|label| self.extra.columns.iter().position(|c| *c == label));
        let list = if let Some(i) = self.extra.column {
            &mut self.extra.column_lists[i]
        } else {
            &mut self.lists[self.tab.index()]
        };
        *list = saved.list;
        // An answer delivered while away must not leave a stale loading token.
        list.loading = None;
        self.threads = saved.threads;
        for (_, list) in &mut self.threads {
            list.loading = None;
        }
        self.profile = saved.profile;
        self.extra.author = saved.author;
        self.extra.relationship = saved.relationship;
        self.search.list = List::default();
    }
}
