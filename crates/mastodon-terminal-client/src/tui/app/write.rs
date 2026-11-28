//! Account actions, confirmations, reports and local composer attachments.
use super::*;
use crate::api::types::Relationship;
use crate::tui::worker::RelationAction;

#[derive(Debug, Clone)]
pub(crate) enum Confirmation {
    Relation { id: String, action: RelationAction },
    Delete(String),
}

#[derive(Debug, Clone)]
pub(crate) struct Report {
    pub account: String,
    pub shown: String,
    pub status: Option<String>,
    pub comment: TextInput,
    pub writing: bool,
    pub confirming: bool,
    pub sending: Option<u64>,
}

#[derive(Debug, Clone)]
pub(crate) struct Attached {
    pub path: std::path::PathBuf,
    pub alt: TextInput,
}
impl Attached {
    pub(super) fn video(&self) -> bool {
        terminal_kit::files::is_video_name(&self.path.to_string_lossy())
    }
}

impl App {
    pub(crate) fn account_target(&self) -> Option<&Account> {
        if self.threads.is_empty() {
            if self.tab == Tab::Profile {
                return self.profile.account.as_ref();
            }
            if self.search_results() && self.search.result_accounts {
                return self.search.users.current();
            }
        }
        self.selected().map(|s| &s.account)
    }

    pub(crate) fn own(&self, account: &Account) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| !s.user_id.is_empty() && s.user_id == account.id)
    }

    pub(super) fn relation_key(&mut self, action: RelationAction) -> Jobs {
        if self.session.is_none() {
            self.needs_account();
            return Vec::new();
        }
        let Some(a) = self.account_target() else {
            return Vec::new();
        };
        if self.own(a) {
            self.say(t("this is your own account"));
            return Vec::new();
        }
        let id = a.id.clone();
        if self.relation_pending.contains_key(&id) {
            return Vec::new();
        }
        if action != RelationAction::Follow {
            self.overlay = Some(Overlay::Confirm(Confirmation::Relation { id, action }));
            return Vec::new();
        }
        self.start_relation(id, action)
    }

    fn start_relation(&mut self, id: String, action: RelationAction) -> Jobs {
        if self.relation_pending.contains_key(&id) {
            return Vec::new();
        }
        self.relation_pending.insert(id.clone(), action);
        if self.relationships.contains_key(&id) {
            self.write_relation(id, action)
        } else {
            vec![self.job(Job::Relationship { id })]
        }
    }

    fn write_relation(&mut self, id: String, action: RelationAction) -> Jobs {
        let Some(before) = self.relationships.get(&id).cloned() else {
            return Vec::new();
        };
        let on = !action.active(&before);
        let mut after = before.clone();
        match action {
            RelationAction::Follow => {
                after.following = on;
                after.requested = false;
            }
            RelationAction::Mute => after.muting = on,
            RelationAction::Block => after.blocking = on,
        }
        self.relation_before.insert(id.clone(), before);
        self.relationships.insert(id.clone(), after);
        vec![self.job(Job::Relate { id, action, on })]
    }

    pub(super) fn relation_loaded(&mut self, id: String, result: Result<Relationship>) -> Jobs {
        match result {
            Ok(r) => {
                self.relationships.insert(id.clone(), r);
                if let Some(action) = self.relation_pending.get(&id).copied() {
                    return self.write_relation(id, action);
                }
            }
            Err(e) => {
                self.relation_pending.remove(&id);
                self.fail(e.message().to_string());
            }
        }
        Vec::new()
    }

    pub(super) fn related(&mut self, id: String, result: Result<Relationship>) {
        self.relation_pending.remove(&id);
        let before = self.relation_before.remove(&id);
        match result {
            Ok(r) => {
                let state = Self::relationship_label(&r);
                self.relationships.insert(id, r);
                self.say(state);
            }
            Err(e) => {
                if let Some(r) = before {
                    self.relationships.insert(id, r);
                }
                self.fail(e.message().to_string());
            }
        }
    }

    pub(crate) fn relationship_label(r: &Relationship) -> String {
        [
            t(if r.requested {
                n!("follow requested")
            } else if r.following {
                n!("following")
            } else {
                n!("not following")
            }),
            t(if r.muting {
                n!("muted")
            } else {
                n!("not muted")
            }),
            t(if r.blocking {
                n!("blocked")
            } else {
                n!("not blocked")
            }),
        ]
        .join(" · ")
    }

    pub(super) fn delete_key(&mut self) {
        if self.session.is_none() {
            self.needs_account();
            return;
        }
        if let Some(s) = self.selected() {
            if !self.own(&s.account) {
                self.say(t("only your own posts can be deleted"));
                return;
            }
            if !self.deleting.contains(&s.id) {
                self.overlay = Some(Overlay::Confirm(Confirmation::Delete(s.id.clone())));
            }
        }
    }

    pub(super) fn confirm_key(&mut self, c: Confirmation, key: KeyEvent) -> Jobs {
        if key.code != KeyCode::Char('y') {
            self.say(t("cancelled"));
            return Vec::new();
        }
        match c {
            Confirmation::Relation { id, action } => self.start_relation(id, action),
            Confirmation::Delete(id) => {
                if !self.deleting.insert(id.clone()) {
                    return Vec::new();
                }
                vec![self.job(Job::Delete { id })]
            }
        }
    }

    pub(super) fn deleted(&mut self, id: String, result: Result<()>) {
        self.deleting.remove(&id);
        match result {
            Ok(()) => {
                self.deleted.insert(id);
                self.say(t("deleted"));
            }
            Err(e) => self.fail(e.message().to_string()),
        }
    }

    /// Remove deleted statuses, including boosts and late pages already in flight.
    pub(super) fn hide_deleted(&mut self) {
        let deleted = &self.deleted;
        let keep = |s: &Status| !deleted.contains(&s.id) && !deleted.contains(&s.shown().id);
        for l in self
            .timelines
            .iter_mut()
            .chain([&mut self.search.posts, &mut self.profile.posts])
        {
            l.items.retain(&keep);
            l.step(0);
        }
        for c in &mut self.deck.columns {
            c.posts.items.retain(&keep);
            c.posts.step(0);
            c.notifications
                .items
                .retain(|n| n.status.as_ref().is_none_or(&keep));
            c.notifications.step(0);
        }
        self.notifications
            .items
            .retain(|n| n.status.as_ref().is_none_or(&keep));
        self.notifications.step(0);
        for th in &mut self.threads {
            th.rows.items.retain(|r| keep(&r.status));
            th.rows.step(0);
        }
    }

    pub(super) fn open_report(&mut self) {
        if self.session.is_none() {
            self.needs_account();
            return;
        }
        let (a, status) = match self.selected() {
            Some(s) => (&s.account, Some(s.id.clone())),
            None => match self.account_target() {
                Some(a) => (a, None),
                None => return,
            },
        };
        if self.own(a) {
            self.say(t("this is your own account"));
            return;
        }
        self.overlay = Some(Overlay::Report(Box::new(Report {
            account: a.id.clone(),
            shown: format!("@{}", a.acct),
            status,
            comment: TextInput::multi(""),
            writing: false,
            confirming: false,
            sending: None,
        })));
    }

    pub(super) fn report_key(&mut self, mut r: Box<Report>, key: KeyEvent) -> Jobs {
        let mut jobs = Vec::new();
        if r.sending.is_none() {
            if r.confirming {
                r.confirming = false;
                if key.code == KeyCode::Char('y') {
                    let job = self.job(Job::Report {
                        account: r.account.clone(),
                        status: r.status.clone(),
                        comment: r.comment.text(),
                    });
                    r.sending = Some(job.0);
                    jobs.push(job);
                }
            } else {
                match key.code {
                    KeyCode::Esc if r.writing => r.writing = false,
                    KeyCode::Esc => return jobs,
                    KeyCode::Tab => r.writing = !r.writing,
                    KeyCode::Enter
                        if !r.writing || key.modifiers.contains(KeyModifiers::CONTROL) =>
                    {
                        r.confirming = true
                    }
                    _ if r.writing => {
                        r.comment.handle_key(key);
                    }
                    _ => {}
                }
            }
        }
        self.overlay = Some(Overlay::Report(r));
        jobs
    }

    pub(super) fn reported(&mut self, seq: u64, result: Result<()>) {
        let open = matches!(&self.overlay, Some(Overlay::Report(r)) if r.sending == Some(seq));
        match result {
            Ok(()) => {
                if open {
                    self.overlay = None;
                }
                self.say(t("reported"));
            }
            Err(e) => {
                if open && let Some(Overlay::Report(r)) = &mut self.overlay {
                    r.sending = None;
                }
                self.fail(e.message().to_string());
            }
        }
    }

    pub(super) fn attachment_key(&mut self, c: &mut Compose, key: KeyEvent) -> bool {
        use terminal_kit::files::{Action, Browser};
        if let Some(browser) = &mut c.browser {
            let action = browser.key(key);
            if action != Action::None {
                self.browse_from = Some(browser.dir.clone());
                c.browser = None;
                if let Action::Choose(paths) = action {
                    let videos = paths
                        .iter()
                        .filter(|p| terminal_kit::files::is_video_name(&p.to_string_lossy()))
                        .count();
                    if videos > 0 && (paths.len() != 1 || !c.media.is_empty()) {
                        self.fail(t("attach pictures or one video, not both"));
                    } else {
                        c.media.extend(paths.into_iter().map(|path| Attached {
                            path,
                            alt: TextInput::single(""),
                        }));
                    }
                }
            }
            return true;
        }
        if !key.modifiers.contains(KeyModifiers::CONTROL) {
            return false;
        }
        match key.code {
            KeyCode::Char('o') => {
                let room = self
                    .limits
                    .max_media_attachments
                    .min(4)
                    .saturating_sub(c.media.len());
                if room == 0 || c.media.iter().any(Attached::video) {
                    self.say(t("no room for more attachments"));
                } else {
                    let dir = self
                        .browse_from
                        .clone()
                        .or_else(|| std::env::current_dir().ok())
                        .unwrap_or_else(|| ".".into());
                    c.browser = Some(Browser::open(&dir, room, c.media.is_empty()));
                }
            }
            KeyCode::Char('x') if !c.media.is_empty() => {
                let index = match c.focus {
                    Field::Alt(i) => i,
                    _ => c.media.len() - 1,
                };
                c.media.remove(index);
                c.focus = Field::Text;
            }
            _ => return false,
        }
        true
    }
}
