//! Reporting, as bsky's `report`: `!` on a note reports its author with
//! the note's address, and on a profile with no note selected the
//! account, to the server's moderators. A reason is chosen, a comment may
//! be written, and nothing is sent until `y` confirms.

use super::*;
use crate::cli::more::{ReportReason, report_comment};
use crate::i18n::{n, tf};
use crate::tui::input::TextInput;

/// A report being made.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Report {
    pub user_id: String,
    /// The account, or the account and the note, as the form names them.
    pub shown: String,
    pub note_url: Option<String>,
    pub reason: usize,
    pub comment: TextInput,
    /// Whether the comment has the keys.
    pub writing: bool,
    /// Asked to confirm: the next key sends it on `y`.
    pub confirming: bool,
    pub sending: bool,
}

impl App {
    /// `!`: report the note selected, or the account the profile shows.
    pub(super) fn open_report(&mut self) {
        if !self.may("write:report-abuse", "report") {
            return;
        }
        let host = self.host().to_string();
        let acct = |u: &crate::api::types::User| match u.host.as_deref().filter(|h| !h.is_empty()) {
            Some(h) => format!("@{}@{h}", u.username),
            None => format!("@{}@{host}", u.username),
        };
        let (user_id, shown, note_url) = if let Some(n) = self.shown_note() {
            let url = crate::cli::format::note_url(&self.origin, &n.id);
            (
                n.user.id.clone(),
                format!("{} ({url})", acct(&n.user)),
                Some(url),
            )
        } else if self.tab == Tab::Profile
            && let Some(u) = self.profile.user.as_ref()
        {
            (u.id.clone(), acct(u), None)
        } else {
            return;
        };
        if Some(user_id.as_str()) == self.my_user_id() {
            self.error(n!("that is your own account"));
            return;
        }
        self.overlay = Some(Overlay::Report(Box::new(Report {
            user_id,
            shown,
            note_url,
            reason: 0,
            comment: TextInput::multi(""),
            writing: false,
            confirming: false,
            sending: false,
        })));
    }

    pub(super) fn report_key(&mut self, key: KeyEvent) -> Jobs {
        let Some(Overlay::Report(r)) = &mut self.overlay else {
            return Vec::new();
        };
        if r.sending {
            return Vec::new();
        }
        if r.confirming {
            r.confirming = false;
            if key.code != KeyCode::Char('y') {
                self.info(n!("not reported: change it, or esc to leave"));
                return Vec::new();
            }
            let reason = ReportReason::ALL[r.reason];
            let comment =
                match report_comment(reason, r.note_url.as_deref(), Some(&r.comment.text())) {
                    Ok(c) => c,
                    Err(e) => {
                        let why = e.message().to_string();
                        self.error(why);
                        return Vec::new();
                    }
                };
            r.sending = true;
            let user_id = r.user_id.clone();
            return vec![self.stamp(Job::Report { user_id, comment })];
        }
        let n = ReportReason::ALL.len();
        match key.code {
            KeyCode::Esc if r.writing => r.writing = false,
            KeyCode::Esc => self.overlay = None,
            KeyCode::Tab => r.writing = !r.writing,
            KeyCode::Enter if !r.writing || key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Err(e) = report_comment(
                    ReportReason::ALL[r.reason],
                    r.note_url.as_deref(),
                    Some(&r.comment.text()),
                ) {
                    let why = e.message().to_string();
                    self.error(why);
                    return Vec::new();
                }
                r.confirming = true;
                let shown = r.shown.clone();
                let host = self.host().to_string();
                self.info(tf(
                    "press y to report {} to the moderators of {}, any other key to go back",
                    &[&shown, &host],
                ));
            }
            _ if r.writing => {
                r.comment.handle_key(key);
            }
            KeyCode::Char('j') | KeyCode::Down => r.reason = (r.reason + 1) % n,
            KeyCode::Char('k') | KeyCode::Up => r.reason = (r.reason + n - 1) % n,
            _ => {}
        }
        Vec::new()
    }

    /// The report sent, or why it was not.
    pub(super) fn reported(&mut self, result: crate::error::Result<()>) {
        match result {
            Ok(()) => {
                let shown = match &self.overlay {
                    Some(Overlay::Report(r)) => r.shown.clone(),
                    _ => String::new(),
                };
                self.overlay = None;
                self.info(tf("reported {}", &[&shown]));
            }
            Err(e) => {
                if let Some(Overlay::Report(r)) = &mut self.overlay {
                    r.sending = false;
                }
                self.error(said(&e));
            }
        }
    }
}
