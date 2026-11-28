//! Locally observed read IDs and columns, keyed by the account's public key.
use crate::{
    config::config_dir,
    error::{Error, Result},
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[derive(Serialize, Deserialize, Default)]
pub(crate) struct ReadState {
    #[serde(default)]
    pub seen: HashSet<String>,
    #[serde(default)]
    pub columns: Vec<String>,
}
impl ReadState {
    fn path(public: &str) -> Result<std::path::PathBuf> {
        crate::api::event::bytes::<32>(public)?;
        Ok(config_dir()?.join(format!("state-{public}.json")))
    }
    pub(crate) fn load(public: &str) -> Result<Self> {
        match std::fs::read(Self::path(public)?) {
            Ok(b) => serde_json::from_slice(&b).map_err(Error::file),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::file(e)),
        }
    }
    pub(crate) fn save(&self, public: &str) -> Result<()> {
        let path = Self::path(public)?;
        let tmp = path.with_extension(format!("{}.tmp", std::process::id()));
        std::fs::write(&tmp, serde_json::to_vec(self).map_err(Error::file)?)
            .map_err(Error::file)?;
        std::fs::rename(tmp, path).map_err(Error::file)
    }
}
