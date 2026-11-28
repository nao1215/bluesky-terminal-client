//! Private protocol and application modules for the command-line client.

pub(crate) mod event;
pub(crate) mod nip19;
pub(crate) mod relay;

use crate::{config::Config, error::Result};
use event::{Event, bytes};
use relay::{Filter, Outcome, Relays};
use std::collections::BTreeSet;

pub(crate) struct Client<'a> {
    pub(crate) config: &'a Config,
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
            value: read.value.into_iter().next(),
            warnings: read.warnings,
        })
    }

    pub(crate) fn timeline(&self, limit: usize) -> Result<Outcome<Vec<Event>>> {
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
        let mut notes = self.relays().query(
            Filter {
                kinds: vec![1],
                authors: authors.into_iter().collect(),
                ..Filter::default()
            },
            limit,
        )?;
        notes.warnings.extend(contacts.warnings);
        Ok(notes)
    }

    pub(crate) fn notifications(&self, limit: usize) -> Result<Outcome<Vec<Event>>> {
        self.relays().query(
            Filter {
                kinds: vec![1],
                mentions: vec![self.config.public.clone()],
                ..Filter::default()
            },
            limit,
        )
    }
}

#[cfg(test)]
mod tests;
