//! Account management intents. Disk and OAuth work are handled by the event loop.
use super::*;

impl App {
    pub(crate) fn take_login_open(&mut self) -> Option<String> {
        self.login_open.take()
    }
    pub(crate) fn logged_in(&mut self, session: Session) {
        self.accounts.retain(|s| s.host() != session.host());
        self.accounts.push(session.clone());
        self.switch_to = Some(session);
    }
    pub(crate) fn take_login(&mut self) -> Option<String> {
        self.login_host.take()
    }
    pub(crate) fn take_logout(&mut self) -> Option<String> {
        self.logout_host.take()
    }
    pub(crate) fn login_active(&self) -> bool {
        matches!(self.overlay, Some(Overlay::Login { waiting: true, .. }))
    }
    pub(crate) fn browser_setting(&self) -> Option<String> {
        self.settings.as_ref().and_then(|s| s.browser.clone())
    }
    pub(crate) fn login_url(&mut self, address: String) {
        if let Some(Overlay::Login { url, .. }) = &mut self.overlay {
            *url = Some(address);
        }
    }
    pub(crate) fn login_failed(&mut self, error: &crate::error::Error) {
        if let Some(Overlay::Login { waiting, .. }) = &mut self.overlay {
            *waiting = false;
        }
        self.fail(error.message().to_string());
    }
    pub(super) fn login_key(
        &mut self,
        key: KeyEvent,
        mut input: TextInput,
        url: Option<String>,
        waiting: bool,
    ) {
        if key.code == KeyCode::Esc {
            self.login_host = None;
            self.overlay = Some(Overlay::Accounts { selected: 0 });
            return;
        }
        if waiting {
            if key.code == KeyCode::Char('o')
                && let Some(url) = &url
            {
                // Opening is a local effect requested through the loop, not network state.
                self.login_open = Some(url.clone());
            }
        } else if key.code == KeyCode::Enter {
            match crate::api::normalize_host(&input.text()) {
                Ok(origin) => {
                    self.login_host = Some(origin);
                    self.overlay = Some(Overlay::Login {
                        input,
                        url: None,
                        waiting: true,
                    });
                    return;
                }
                Err(e) => self.fail(e.message().to_string()),
            }
        } else {
            input.handle_key(key);
        }
        self.overlay = Some(Overlay::Login {
            input,
            url,
            waiting,
        });
    }

    /// Forget all state belonging to a logged-out account, including late replies.
    pub(crate) fn logged_out(&mut self, host: &str) -> Jobs {
        self.accounts.retain(|s| s.host() != host);
        if !self.session.as_ref().is_some_and(|s| s.host() == host) {
            self.overlay = Some(Overlay::Accounts { selected: 0 });
            return Vec::new();
        }
        if let Some(next) = self.accounts.first().cloned() {
            return self.switched(next);
        }
        let settings = self.settings.clone();
        let seq = self.seq;
        let origin = self.origin.clone();
        let (mut fresh, _) = Self::new(None, &origin);
        fresh.seq = seq;
        fresh.apply_settings(settings, self.depth);
        fresh.overlay = Some(Overlay::Accounts { selected: 0 });
        *self = fresh;
        self.load_tab(Tab::Local, false)
    }
}
