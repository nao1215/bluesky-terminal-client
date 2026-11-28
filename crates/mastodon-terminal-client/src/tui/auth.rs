//! Cancellable browser OAuth. The UI saves the result only while it is still wanted.
use crate::{
    api::{self, Client},
    config::{AccountStore, Session},
    error::{Error, Result},
    oauth,
};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::{self, Receiver},
};

pub(super) enum Update {
    Url(String),
    Done(Result<Session>),
}
pub(super) struct Login {
    pub receiver: Receiver<Update>,
    cancel: Arc<AtomicBool>,
}
impl Drop for Login {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
impl Login {
    pub(super) fn start(
        store: AccountStore,
        origin: String,
        browser: Option<String>,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let cancelled = Arc::clone(&cancel);
        std::thread::Builder::new()
            .name("mstdn-login".into())
            .spawn(move || {
                let result = (|| {
                    let client = Client::new(&origin);
                    let (app, redirect) = oauth::app_for(&store, &client, false)?;
                    let uri = redirect.uri()?;
                    let state = api::random_hex("")?;
                    let pkce = oauth::pkce()?;
                    let url = oauth::authorize_url(
                        &origin,
                        &app.client_id,
                        &uri,
                        &state,
                        &pkce.challenge,
                    );
                    if cancelled.load(Ordering::Relaxed) {
                        return Err(Error::usage("login cancelled"));
                    }
                    sender
                        .send(Update::Url(url.clone()))
                        .map_err(|_disconnected| Error::usage("login cancelled"))?;
                    crate::browser::open_with(&url, browser.as_deref());
                    let oauth::Redirect::Loopback(listener) = redirect else {
                        return Err(Error::usage("browser login needs a loopback callback"));
                    };
                    let code = oauth::wait_for_code_until(
                        &listener,
                        &state,
                        client.host(),
                        std::time::Duration::from_secs(300),
                        || cancelled.load(Ordering::Relaxed),
                    )?;
                    if cancelled.load(Ordering::Relaxed) {
                        return Err(Error::usage("login cancelled"));
                    }
                    let token =
                        oauth::exchange(&store, &client, &app, &code, &uri, &pkce.verifier)?;
                    oauth::session(&client, token)
                })();
                let _sent = sender.send(Update::Done(result));
            })
            .map_err(|e| Error::io(format!("cannot start login: {e}")))?;
        Ok(Self { receiver, cancel })
    }
}
