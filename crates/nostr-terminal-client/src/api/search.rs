//! Search uses the shared relay capability checks, limits and pagination.
use super::{
    Client,
    event::Event,
    relay::{Filter, Outcome, Relays},
};
use crate::error::{Error, Result};
impl Client<'_> {
    pub(crate) fn search(&self, text: &str, count: usize) -> Result<Outcome<Vec<Event>>> {
        let text = text.trim();
        if text.is_empty() || text == "#" {
            return Err(Error::usage("search query must not be empty"));
        }
        let mut filter = Filter {
            kinds: vec![1],
            ..Filter::default()
        };
        let urls = if let Some(tag) = text.strip_prefix('#') {
            filter.hashtags.push(tag.into());
            &self.config.relays
        } else {
            if self.config.search_relays.is_empty() {
                return Err(Error::unavailable(
                    "unsupported: NIP-50 search requires searchRelays in config.json",
                ));
            }
            filter.search = Some(text.into());
            &self.config.search_relays
        };
        Relays {
            urls,
            timeout: self.config.timeout,
        }
        .query(filter, count)
    }
}
