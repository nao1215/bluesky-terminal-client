//! Batch author metadata; incomplete metadata never discards an otherwise valid page.
use super::{
    Client,
    event::Event,
    relay::{Filter, Outcome},
};
use crate::{error::Result, output};
use serde_json::Value;
use std::collections::{BTreeSet, HashMap};

impl Client<'_> {
    pub(crate) fn metadata(
        &self,
        authors: BTreeSet<String>,
    ) -> Result<Outcome<HashMap<String, String>>> {
        let out = self.relays().query(
            Filter {
                kinds: vec![0],
                authors: authors.into_iter().collect(),
                ..Filter::default()
            },
            1000,
        )?;
        let mut values = HashMap::new();
        for event in out.value {
            values.entry(event.pubkey).or_insert(event.content);
        }
        Ok(Outcome {
            value: values,
            warnings: out.warnings,
            answered: out.answered,
        })
    }
    pub(crate) fn enrich(&self, values: &mut [Value], warnings: &mut Vec<String>) {
        fn collect(value: &Value, authors: &mut BTreeSet<String>) {
            if let Some(id) = value.get("user").and_then(|u| u["id"].as_str()) {
                authors.insert(id.into());
            }
            for key in ["post", "repostOf", "lastMessage"] {
                if let Some(v) = value.get(key) {
                    collect(v, authors);
                }
            }
        }
        fn apply(value: &mut Value, profiles: &HashMap<String, String>) {
            if let Some(user) = value.get_mut("user")
                && let Some(id) = user["id"].as_str()
                && let Some(content) = profiles.get(id)
                && let Ok(profile) = serde_json::from_str::<Value>(content)
            {
                user["name"] = profile
                    .get("display_name")
                    .or_else(|| profile.get("name"))
                    .cloned()
                    .unwrap_or(Value::Null);
            }
            for key in ["post", "repostOf", "lastMessage"] {
                if let Some(v) = value.get_mut(key) {
                    apply(v, profiles);
                }
            }
        }
        let mut authors = BTreeSet::new();
        for v in values.iter() {
            collect(v, &mut authors);
        }
        if authors.is_empty() {
            return;
        }
        match self.metadata(authors) {
            Ok(out) => {
                warnings.extend(out.warnings);
                for v in values {
                    apply(v, &out.value);
                }
            }
            Err(e) => warnings.push(format!("author metadata unavailable: {}", e.error)),
        }
    }
    pub(crate) fn posts(&self, out: Outcome<Vec<Event>>) -> Result<(Vec<Value>, Vec<String>)> {
        let mut values = out
            .value
            .iter()
            .map(output::post)
            .collect::<Result<Vec<_>>>()?;
        let mut warnings = out.warnings;
        self.enrich(&mut values, &mut warnings);
        Ok((values, warnings))
    }
}
