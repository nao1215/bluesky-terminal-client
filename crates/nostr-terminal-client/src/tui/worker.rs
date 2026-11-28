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
    Author(String),
    Search(String),
}

pub(super) enum Job {
    Task {
        seq: u64,
        operation: super::extras::Operation,
    },
    Prefetch {
        seq: u64,
        note: Event,
    },
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
    Task(u64, Result<serde_json::Value>),
    Prefetched(u64, Result<Outcome<Vec<Event>>>),
    Metadata(std::collections::HashMap<String, String>),
    Referenced(Vec<Event>),
    Relationship(u64, String, bool, bool),
    Marks(u64, Vec<String>, Vec<Event>),
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
        let writing = matches!(job, Job::Write { .. } | Job::Task { .. });
        if writing {
            writes.start();
        }
        let spawned = std::thread::Builder::new()
            .name("nstr-relay".into())
            .spawn(move || {
                let profile_author = match &job {
                    Job::Load {
                        target: Target::Author(p),
                        ..
                    } => Some(p.clone()),
                    _ => None,
                };
                let answer = match job {
                    Job::Task { seq, operation } => {
                        let auth = |url: &str| {
                            if send.send(Answer::Authorize(url.into())).is_err() { /* UI closed. */
                            }
                        };
                        let mut client = Client::new(&config);
                        client.authorize = Some(&auth);
                        Answer::Task(seq, operation.execute(&client))
                    }
                    Job::Prefetch { seq, note } => {
                        Answer::Prefetched(seq, Client::new(&config).thread(&note.id, 20))
                    }
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
                let loaded_seq = match &answer {
                    Answer::Loaded(seq, _) => *seq,
                    _ => 0,
                };
                let note_ids = match &answer {
                    Answer::Loaded(_, Ok(out)) => {
                        out.value.notes.iter().map(|e| e.id.clone()).collect()
                    }
                    _ => Vec::new(),
                };
                let authors = match &answer {
                    Answer::Loaded(_, Ok(out)) => out
                        .value
                        .notes
                        .iter()
                        .map(|e| e.pubkey.clone())
                        .collect::<std::collections::BTreeSet<_>>(),
                    _ => Default::default(),
                };
                let quotes: Vec<_> = match &answer {
                    Answer::Loaded(_, Ok(out)) => out
                        .value
                        .notes
                        .iter()
                        .flat_map(|e| e.tag_values("q"))
                        .filter(|id| crate::api::event::bytes::<32>(id).is_ok())
                        .map(str::to_owned)
                        .collect(),
                    _ => vec![],
                };
                if send.send(answer).is_err() { /* The client has already closed. */ }
                if writing {
                    writes.done();
                }
                if !quotes.is_empty()
                    && let Ok(out) = Client::new(&config).relays().query(
                        Filter {
                            kinds: vec![1],
                            ids: quotes,
                            ..Filter::default()
                        },
                        1000,
                    )
                    && send.send(Answer::Referenced(out.value)).is_err()
                { /* UI closed. */
                }
                if !note_ids.is_empty()
                    && let Ok(out) = (Client::new(&config)).active_actions(note_ids.clone())
                    && send
                        .send(Answer::Marks(loaded_seq, note_ids, out.value))
                        .is_err()
                { /* UI closed. */ }
                if let Some(author) = profile_author {
                    let client = Client::new(&config);
                    if let (Ok(follow), Ok(mute)) = (client.latest(3), client.latest(10000)) {
                        let follows = follow
                            .value
                            .is_some_and(|e| e.tag_values("p").any(|p| p == author));
                        let muted = mute
                            .value
                            .is_some_and(|e| e.tag_values("p").any(|p| p == author));
                        if send
                            .send(Answer::Relationship(loaded_seq, author, follows, muted))
                            .is_err()
                        { /* UI closed. */ }
                    }
                }
                if !authors.is_empty()
                    && let Ok(out) = (Client::new(&config)).metadata(authors)
                    && send.send(Answer::Metadata(out.value)).is_err()
                { /* UI closed. */
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
    if unsigned.kind == 6 {
        for t in &mut unsigned.tags {
            if t.first().is_some_and(|s| s == "e") {
                if t.len() == 2 {
                    t.push(config.relays[0].clone());
                } else if t[2].is_empty() {
                    t[2] = config.relays[0].clone();
                }
            }
        }
    }
    let auth = |url: &str| {
        if send.send(Answer::Authorize(url.into())).is_err() { /* UI closed. */ }
    };
    let mut client = Client::new(config);
    client.authorize = Some(&auth);
    let signed = client.sign(unsigned)?;
    client.relays().publish(&signed, Some(&client))
}

fn load(
    config: &Config,
    target: Target,
    count: usize,
    until: Option<u64>,
) -> Result<Outcome<Page>> {
    let client = Client::new(config);
    let mut warnings = Vec::new();
    let mut prefix = Vec::new();
    let mut profile = None;
    let mut filter = Filter {
        kinds: vec![1],
        ..Filter::default()
    };
    match &target {
        Target::Search(text) => {
            let out = client.search(text, count)?;
            return Ok(Outcome {
                answered: out.answered,
                warnings: out.warnings,
                value: Page {
                    notes: out.value,
                    profile: None,
                    more: false,
                    append: false,
                    until: None,
                    boundary: 0,
                },
            });
        }
        Target::Author(author) => {
            let metadata = client.profile(author)?;
            profile = metadata.value.map(|e| e.content);
            warnings.extend(metadata.warnings);
            filter.authors = vec![author.clone()];
        }
        Target::Tab(Tab::Search | Tab::Chat) => {
            return Err(Error::usage("select a search or chat first"));
        }
        Target::Tab(Tab::Home) => {
            let home = client.home_filter()?;
            warnings.extend(home.warnings);
            filter = home.value;
        }
        Target::Tab(Tab::Notifications) => {
            filter.kinds = vec![1, 3, 6, 7, 16];
            filter.mentions = vec![config.public.clone()];
        }
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
            if until.is_none() {
                let out = client.thread(&note.id, count)?;
                let replies: Vec<_> = out
                    .value
                    .iter()
                    .filter(|e| e.reply_id() == Some(note.id.as_str()))
                    .collect();
                let more = replies.len() >= count;
                let until = replies.iter().map(|e| e.created_at).min();
                let boundary = replies
                    .iter()
                    .filter(|e| Some(e.created_at) == until)
                    .count();
                return Ok(Outcome {
                    answered: out.answered,
                    warnings: out.warnings,
                    value: Page {
                        notes: out.value,
                        profile: None,
                        more,
                        append: false,
                        until,
                        boundary,
                    },
                });
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
    if matches!(target, Target::Tab(Tab::Home | Tab::Notifications)) {
        found = client.visible(found)?;
    }
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
