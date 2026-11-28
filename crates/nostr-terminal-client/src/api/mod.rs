//! Private protocol and application modules for the command-line client.

pub(crate) mod actions;
pub(crate) mod dm;
pub(crate) mod event;
pub(crate) mod media;
mod nip11;
pub(crate) mod nip19;
pub(crate) mod nip44;
pub(crate) mod nip46;
pub(crate) mod profiles;
pub(crate) mod relay;
pub(crate) mod search;

use crate::{config::Config, error::Result};
use event::{Event, bytes};
use relay::{Filter, Outcome, Relays};
use std::collections::BTreeSet;

pub(crate) struct Client<'a> {
    pub(crate) config: &'a Config,
    pub(crate) authorize: Option<&'a (dyn Fn(&str) + Sync)>,
}
impl<'a> Client<'a> {
    pub(crate) fn new(config: &'a Config) -> Self {
        Self {
            config,
            authorize: None,
        }
    }
    pub(crate) fn authorization(&self, url: &str) {
        if let Some(notify) = self.authorize {
            notify(url);
        } else {
            crate::diagnostics(&format!(
                "Remote signer authorization required. Open this URL yourself: {url}"
            ));
        }
    }
}

impl Client<'_> {
    pub(crate) fn relays(&self) -> Relays<'_> {
        Relays {
            urls: &self.config.relays,
            timeout: self.config.timeout,
        }
    }

    pub(crate) fn latest(&self, kind: u16) -> Result<Outcome<Option<Event>>> {
        let read = self.relays().query(
            Filter {
                kinds: vec![kind],
                authors: vec![self.config.public.clone()],
                ..Filter::default()
            },
            1,
        )?;
        Ok(Outcome {
            answered: read.answered,
            value: read.value.into_iter().next(),
            warnings: read.warnings,
        })
    }

    pub(crate) fn home_filter(&self) -> Result<Outcome<Filter>> {
        let contacts = self.latest(3)?;
        let mut authors = BTreeSet::from([self.config.public.clone()]);
        if let Some(event) = contacts.value {
            authors.extend(
                event
                    .tag_values("p")
                    .filter(|p| bytes::<32>(p).is_ok())
                    .map(str::to_owned),
            );
        }
        Ok(Outcome {
            value: Filter {
                kinds: vec![1],
                authors: authors.into_iter().collect(),
                ..Filter::default()
            },
            warnings: contacts.warnings,
            answered: contacts.answered,
        })
    }

    pub(crate) fn timeline(&self, limit: usize) -> Result<Outcome<Vec<Event>>> {
        let filter = self.home_filter()?;
        let mut notes = self.relays().query(filter.value, limit)?;
        notes.warnings.extend(filter.warnings);
        self.visible(notes)
    }

    pub(crate) fn notifications(&self, limit: usize) -> Result<Outcome<Vec<Event>>> {
        let notes = self.relays().query(
            Filter {
                kinds: vec![1, 3, 6, 7, 16],
                mentions: vec![self.config.public.clone()],
                ..Filter::default()
            },
            limit,
        )?;
        self.visible(notes)
    }
}

#[cfg(test)]
mod tests;
