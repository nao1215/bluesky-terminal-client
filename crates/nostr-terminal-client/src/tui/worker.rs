//! Blocking relay and signer sessions live outside the drawing thread.
use super::app::Tab;
use crate::{
    api::{
        Client,
        event::{Event, Unsigned},
        relay::{Filter, Outcome},
    },
    config::Config,
    error::{Error, Result},
};
use std::sync::{Arc, mpsc};
use terminal_kit::inflight::InFlight;

#[derive(Clone, Debug)]
pub(super) enum Target {
    Tab(Tab),
    Thread(Event),
}

pub(super) enum Job {
    Load {
        seq: u64,
        target: Target,
        count: usize,
        until: Option<u64>,
    },
    Write {
        seq: u64,
        event: Unsigned,
    },
}

pub(super) struct Page {
    pub notes: Vec<Event>,
    pub profile: Option<String>,
    pub more: bool,
    pub append: bool,
    pub until: Option<u64>,
    pub boundary: usize,
}

pub(super) enum Answer {
    Loaded(u64, Result<Outcome<Page>>),
    Written(u64, Result<Outcome<usize>>),
    Authorize(String),
}

pub(super) struct Worker {
    config: Arc<Config>,
    tx: mpsc::Sender<Answer>,
    pub rx: mpsc::Receiver<Answer>,
    pub writes: InFlight,
}

impl Worker {
    pub(super) fn new(config: Config) -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            config: Arc::new(config),
            tx,
            rx,
            writes: InFlight::default(),
        }
    }
    pub(super) fn send(&self, job: Job) -> Result<()> {
        let config = self.config.clone();
        let send = self.tx.clone();
        let writes = self.writes.clone();
        let writing = matches!(job, Job::Write { .. });
        if writing {
            writes.start();
        }
        let spawned = std::thread::Builder::new()
            .name("nstr-relay".into())
            .spawn(move || {
                let answer = match job {
                    Job::Load {
                        seq,
                        target,
                        count,
                        until,
                    } => Answer::Loaded(seq, load(&config, target, count, until)),
                    Job::Write { seq, event } => {
                        Answer::Written(seq, publish(&config, event, &send))
                    }
                };
                if send.send(answer).is_err() { /* The client has already closed. */ }
                if writing {
                    writes.done();
                }
            });
        if let Err(e) = spawned {
            if writing {
                self.writes.done();
            }
            return Err(Error::file(e));
        }
        Ok(())
    }
}

fn publish(
    config: &Config,
    mut unsigned: Unsigned,
    send: &mpsc::Sender<Answer>,
) -> Result<Outcome<usize>> {
    unsigned.created_at =
        u64::try_from(jiff::Timestamp::now().as_second()).map_err(Error::usage)?;
    let signed = if let Some(bunker) = &config.bunker {
        bunker.sign_event(&config.public, &unsigned, config.timeout, &|url| {
            if send.send(Answer::Authorize(url.into())).is_err() { /* The client has closed. */ }
        })?
    } else {
        Event::sign(
            &config.signing_key()?,
            unsigned.created_at,
            unsigned.kind,
            unsigned.tags,
            unsigned.content,
        )?
    };
    Client { config }.relays().publish(&signed)
}

fn load(
    config: &Config,
    target: Target,
    count: usize,
    until: Option<u64>,
) -> Result<Outcome<Page>> {
    let client = Client { config };
    let mut warnings = Vec::new();
    let mut prefix = Vec::new();
    let mut profile = None;
    let mut filter = Filter {
        kinds: vec![1],
        ..Filter::default()
    };
    match &target {
        Target::Tab(Tab::Home) => {
            let home = client.home_filter()?;
            warnings.extend(home.warnings);
            filter = home.value;
        }
        Target::Tab(Tab::Notifications) => filter.mentions = vec![config.public.clone()],
        Target::Tab(Tab::Profile) => {
            let metadata = client.latest(0)?;
            warnings.extend(metadata.warnings);
            profile = Some(
                metadata
                    .value
                    .map(|event| event.content)
                    .unwrap_or_default(),
            );
            filter.authors = vec![config.public.clone()];
        }
        Target::Thread(note) => {
            if until.is_none()
                && let Some(parent) = note.reply_id()
            {
                match client.relays().query(
                    Filter {
                        kinds: vec![1],
                        ids: vec![parent.into()],
                        ..Filter::default()
                    },
                    1,
                ) {
                    Ok(found) => {
                        prefix.extend(found.value);
                        warnings.extend(found.warnings);
                    }
                    Err(e) => warnings.push(e.error),
                }
            }
            if until.is_none() {
                prefix.push(note.clone());
            }
            filter.references = vec![note.id.clone()];
        }
    }
    // The next page starts at the previous oldest timestamp. Its limit
    // includes that boundary's events, so ties cannot crowd out older notes.
    filter.until = until;
    let mut found = client.relays().query(filter, count)?;
    let more = found.value.len() >= count;
    let next_until = found.value.iter().map(|e| e.created_at).min();
    let boundary = found
        .value
        .iter()
        .filter(|e| Some(e.created_at) == next_until)
        .count();
    if let Target::Thread(note) = target {
        found
            .value
            .retain(|e| e.reply_id() == Some(note.id.as_str()));
    }
    prefix.extend(found.value);
    let mut seen = std::collections::HashSet::new();
    prefix.retain(|e| seen.insert(e.id.clone()));
    warnings.extend(found.warnings);
    Ok(Outcome {
        answered: found.answered,
        warnings,
        value: Page {
            notes: prefix,
            profile,
            more,
            append: until.is_some(),
            until: next_until,
            boundary,
        },
    })
}
