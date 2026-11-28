//! UI preferences stay separate from account credentials.
use crate::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

#[derive(Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Settings {
    pub theme: Option<String>,
    pub language: Option<String>,
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

impl Settings {
    pub(super) fn load(path: &Path) -> Result<Self> {
        match fs::read(path) {
            Ok(data) => serde_json::from_slice(&data).map_err(Error::file),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(Error::file(e)),
        }
    }
    pub(super) fn save(&self, path: &Path) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(Error::file)?;
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, bytes).map_err(Error::file)?;
        fs::rename(tmp, path).map_err(Error::file)
    }
    pub(super) fn language(&self) -> &'static str {
        self.language
            .as_deref()
            .and_then(terminal_kit::locale::language_code)
            .unwrap_or_else(|| {
                terminal_kit::locale::language_from_locale(|key| std::env::var(key).ok())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn settings_roundtrip_preserves_unknown_fields_and_rejects_invalid_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        assert!(Settings::load(&path).unwrap().theme.is_none());
        fs::write(&path, r#"{"theme":"nord","language":"ja","extra":7}"#).unwrap();
        let mut settings = Settings::load(&path).unwrap();
        assert_eq!(settings.language(), "ja");
        settings.theme = Some("terminal".into());
        settings.save(&path).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(value["extra"], 7);
        assert_eq!(value["theme"], "terminal");
        fs::write(&path, "broken").unwrap();
        assert!(Settings::load(&path).is_err());
        assert_eq!(fs::read_to_string(&path).unwrap(), "broken");
    }
}
