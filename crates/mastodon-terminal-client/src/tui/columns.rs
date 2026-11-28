//! Independent sources and paging state for the Home deck.
use super::list::List;
use crate::api::types::{Notification, Status};
use crate::i18n::t;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Source {
    Home,
    List { id: String, title: String },
    Tag(String),
    Notifications,
    Search(String),
}
impl Source {
    pub(crate) fn title(&self) -> String {
        match self {
            Self::Home => t("Home").into(),
            Self::List { title, .. } => title.clone(),
            Self::Tag(tag) => format!("#{tag}"),
            Self::Notifications => t("Notifications").into(),
            Self::Search(q) => format!("{}: {q}", t("Search")),
        }
    }
}
#[derive(Debug, Clone)]
pub(crate) struct Column {
    pub id: u64,
    pub source: Source,
    pub posts: List<Status>,
    pub notifications: List<Notification>,
}
impl Column {
    pub(crate) fn selected(&self) -> Option<&Status> {
        if self.source == Source::Notifications {
            self.notifications.current().and_then(|n| n.status.as_ref())
        } else {
            self.posts.current()
        }
        .map(Status::shown)
    }
}
#[derive(Debug, Default)]
pub(crate) struct Deck {
    pub columns: Vec<Column>,
    pub focus: usize,
    pub choices: Vec<Source>,
    pub choices_waiting: Option<u64>,
    next_id: u64,
}
impl Deck {
    pub(crate) fn add(&mut self, source: Source) -> u64 {
        self.next_id += 1;
        let id = self.next_id;
        self.columns.push(Column {
            id,
            source,
            posts: List::default(),
            notifications: List::default(),
        });
        self.focus = self.columns.len() - 1;
        id
    }
}
