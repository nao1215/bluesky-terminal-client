//! Several accounts: the account list, switching to one, and logging one out.

use super::*;

impl App {
    /// Where the account in use is in the account list.
    pub(super) fn current_account_index(&self) -> Option<usize> {
        let did = &self.session.as_ref()?.did;
        self.accounts.iter().position(|a| a.did == *did)
    }

    /// Put `session`'s account in the list, or bring its handle up to date.
    pub(super) fn remember_account(&mut self, session: &Session) {
        let account = Account::from(session);
        match self.accounts.iter_mut().find(|a| a.did == account.did) {
            Some(a) => *a = account,
            None => self.accounts.push(account),
        }
        self.accounts.sort_by(|a, b| {
            (a.handle.to_lowercase(), &a.did).cmp(&(b.handle.to_lowercase(), &b.did))
        });
    }

    /// A key on the account list.
    pub(super) fn accounts_key(&mut self, key: KeyEvent, selected: usize) {
        let n = self.accounts.len().max(1);
        let selected = selected.min(n - 1);
        // The question x asked takes the next key, as D's does.
        if let Some(did) = self.confirm_logout.take() {
            if key.code == KeyCode::Char('y') {
                self.overlay = None;
                self.settings_return = None;
                self.info("logging out…");
                self.account_logout = Some(did);
            } else {
                self.info("still logged in");
            }
            return;
        }
        let at = |selected| Some(Overlay::Accounts { selected });
        match key.code {
            KeyCode::Char('j') | KeyCode::Down => self.overlay = at((selected + 1) % n),
            KeyCode::Char('k') | KeyCode::Up => self.overlay = at((selected + n - 1) % n),
            // Opened from the settings, esc goes back there; anything done
            // with an account closes both.
            KeyCode::Esc | KeyCode::Char('q' | 'A') => {
                self.overlay = self
                    .settings_return
                    .take()
                    .map(|selected| Overlay::Settings {
                        selected,
                        edit: None,
                    });
            }
            KeyCode::Enter => {
                self.overlay = None;
                self.settings_return = None;
                if let Some(a) = self.accounts.get(selected).cloned()
                    && Some(selected) != self.current_account_index()
                {
                    self.info(format!("switching to @{}…", a.handle));
                    self.account_switch = Some(a.did);
                }
            }
            KeyCode::Char('a') => {
                self.overlay = None;
                self.settings_return = None;
                let service = self
                    .session
                    .as_ref()
                    .map(|s| s.service.clone())
                    .unwrap_or_else(|| crate::api::DEFAULT_SERVICE.to_string());
                let mut form = LoginForm::new(&service);
                form.adding = true;
                self.login = Some(form);
            }
            KeyCode::Char('x') => {
                if let Some(a) = self.accounts.get(selected).cloned() {
                    self.info(format!(
                        "press y to log out @{}, any other key to stay logged in",
                        a.handle
                    ));
                    self.confirm_logout = Some(a.did);
                    self.asked();
                }
            }
            _ => {}
        }
    }

    /// Why an account could not be switched to or logged out.
    pub fn account_error(&mut self, why: String) {
        self.error(why);
    }

    /// The account the list asked to switch to, for the event loop.
    pub fn take_account_switch(&mut self) -> Option<String> {
        self.account_switch.take()
    }

    /// The account the list asked to log out, for the event loop.
    pub fn take_account_logout(&mut self) -> Option<String> {
        self.account_logout.take()
    }

    /// Use `session`'s account from now on: what was loaded for the one
    /// before goes, as it does when another account logs in, and the new
    /// one's lists are asked for. The event loop has given the worker the
    /// session already, so nothing is asked of the account before.
    pub fn switched_to(&mut self, session: Session) -> Vec<Job> {
        let other = self.session.as_ref().is_none_or(|s| s.did != session.did);
        self.remember_account(&session);
        self.info(format!("now @{}", session.handle));
        self.login = None;
        if !other {
            self.session = Some(session);
            return Vec::new();
        }
        self.forget_account();
        self.account_since = self.sent + 1;
        self.session = Some(session);
        self.load_columns();
        let mut jobs = self.startup_jobs();
        jobs.extend(self.settle_timeline());
        self.pending += jobs.len();
        jobs
    }

    /// The account `did` was logged out; `next` is the one in use now, if
    /// any is left. Without one, the login form comes back.
    pub fn logged_out(&mut self, did: &str, next: Option<Session>) -> Vec<Job> {
        let handle = self
            .accounts
            .iter()
            .find(|a| a.did == did)
            .map(|a| a.handle.clone())
            .unwrap_or_default();
        self.accounts.retain(|a| a.did != did);
        let was_current = self.session.as_ref().is_some_and(|s| s.did == did);
        if !was_current {
            self.info(format!("logged out @{handle}"));
            return Vec::new();
        }
        match next {
            Some(s) => {
                let jobs = self.switched_to(s);
                let now = self
                    .session
                    .as_ref()
                    .map(|s| s.handle.clone())
                    .unwrap_or_default();
                self.info(format!("logged out @{handle}; now @{now}"));
                jobs
            }
            None => {
                let service = self.session.take().map(|s| s.service).unwrap_or_default();
                self.forget_account();
                self.account_since = self.sent + 1;
                self.login = Some(LoginForm::new(&service));
                Vec::new()
            }
        }
    }
}
