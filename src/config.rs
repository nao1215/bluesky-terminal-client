//! Where bs keeps its state on disk: the login session and the settings.
//!
//! Both files live in the config directory: `$BS_CONFIG_DIR` when set,
//! otherwise `<platform config dir>/bs` (`$XDG_CONFIG_HOME/bs` or
//! `~/.config/bs` on Linux, `~/Library/Application Support/bs` on macOS,
//! `%APPDATA%\bs` on Windows).
//!
//! - `session.json` holds the tokens of an app-password login, so it is
//!   written with owner-only permissions on Unix. `bs logout` removes it.
//! - `settings.json` holds preferences (the color theme). It is written only
//!   when a preference is changed, and a broken one is ignored with a
//!   warning rather than stopping bs.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Environment variable that overrides the config directory.
pub const CONFIG_DIR_ENV: &str = "BS_CONFIG_DIR";

const SESSION_FILE: &str = "session.json";
const SETTINGS_FILE: &str = "settings.json";

/// User preferences.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Name of the color theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Keys this version of bs does not know, kept so that saving does not
    /// drop what a newer version wrote.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
}

/// Reads and writes `settings.json` inside one directory.
#[derive(Debug, Clone)]
pub struct SettingsStore {
    dir: PathBuf,
}

impl SettingsStore {
    /// A store rooted at `dir`; nothing is touched until a read or write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Path of the settings file.
    pub fn path(&self) -> PathBuf {
        self.dir.join(SETTINGS_FILE)
    }

    /// Load the settings. A missing file is the defaults; a file that cannot
    /// be read or parsed is the defaults too, with a warning to show, because
    /// a preference is not worth refusing to start over.
    pub fn load(&self) -> (Settings, Option<String>) {
        let path = self.path();
        match fs::read(&path) {
            Ok(data) => match serde_json::from_slice(&data) {
                Ok(settings) => (settings, None),
                Err(e) => (
                    Settings::default(),
                    Some(format!(
                        "{} is not valid and was ignored: {e}",
                        path.display()
                    )),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), None),
            Err(e) => (
                Settings::default(),
                Some(format!("cannot read {}: {e}", path.display())),
            ),
        }
    }

    /// Save the settings, creating the directory when needed.
    pub fn save(&self, settings: &Settings) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| Error::io(format!("cannot create {}: {e}", self.dir.display())))?;
        let path = self.path();
        let mut json = serde_json::to_vec_pretty(settings).expect("settings serialize");
        json.push(b'\n');
        write_private(&path, &json)
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))
    }
}

/// An authenticated session against one PDS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    /// Base URL of the PDS the session belongs to, without a trailing slash.
    pub service: String,
    /// Account DID.
    pub did: String,
    /// Account handle at login time.
    pub handle: String,
    /// Short-lived bearer token.
    pub access_jwt: String,
    /// Long-lived token used to mint a new access token.
    pub refresh_jwt: String,
}

/// Resolve the config directory from the environment.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    dirs::config_dir().map(|d| d.join("bs")).ok_or_else(|| {
        Error::io("cannot determine the config directory")
            .with_hint(format!("set {CONFIG_DIR_ENV} to a writable directory"))
    })
}

/// Reads and writes the session file inside one directory.
#[derive(Debug, Clone)]
pub struct SessionStore {
    dir: PathBuf,
}

impl SessionStore {
    /// A store rooted at `dir`; nothing is touched until a read or write.
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// Path of the session file.
    pub fn path(&self) -> PathBuf {
        self.dir.join(SESSION_FILE)
    }

    /// Load the saved session; `Ok(None)` when there is none yet.
    pub fn load(&self) -> Result<Option<Session>> {
        let path = self.path();
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::io(format!("cannot read {}: {e}", path.display()))),
        };
        serde_json::from_slice(&data).map(Some).map_err(|e| {
            Error::io(format!(
                "{} is not a valid session file: {e}",
                path.display()
            ))
            .with_hint("run `bs logout` to discard it and log in again")
        })
    }

    /// Save the session, creating the directory when needed.
    pub fn save(&self, session: &Session) -> Result<()> {
        fs::create_dir_all(&self.dir)
            .map_err(|e| Error::io(format!("cannot create {}: {e}", self.dir.display())))?;
        let path = self.path();
        let json = serde_json::to_vec_pretty(session).expect("session serializes");
        write_private(&path, &json)
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))
    }

    /// Remove the saved session. Returns whether a session existed.
    pub fn clear(&self) -> Result<bool> {
        let path = self.path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(Error::io(format!("cannot remove {}: {e}", path.display()))),
        }
    }
}

/// Write `data` to `path` so that a crash leaves either the old file or the
/// new one, never a truncated one: the refresh token is rotated on every
/// refresh, and losing the new one means logging in again.
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    // The process id keeps two running copies of bs from writing the same
    // temporary file, which on Windows would make one of the renames fail.
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut file = open_private(&tmp)?;
    file.write_all(data)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&tmp, path)
}

#[cfg(unix)]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // A leftover file keeps its old mode through open(); tighten it explicitly.
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

#[cfg(not(unix))]
fn open_private(path: &Path) -> std::io::Result<fs::File> {
    fs::File::create(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Session {
        Session {
            service: "https://pds.example".into(),
            did: "did:plc:alice".into(),
            handle: "alice.test".into(),
            access_jwt: "access".into(),
            refresh_jwt: "refresh".into(),
        }
    }

    #[test]
    fn load_without_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(SessionStore::new(dir.path()).load().unwrap(), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path().join("nested"));
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap(), Some(sample()));
    }

    #[test]
    fn clear_reports_whether_a_session_existed() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        assert!(!store.clear().unwrap());
        store.save(&sample()).unwrap();
        assert!(store.clear().unwrap());
        assert_eq!(store.load().unwrap(), None);
    }

    #[test]
    fn corrupt_file_is_an_io_error_with_a_hint() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        fs::write(store.path(), b"{not json").unwrap();
        let err = store.load().unwrap_err();
        assert_eq!(err.kind(), crate::error::Kind::Io);
        assert!(err.to_string().contains("\nhint: run `bs logout`"), "{err}");
    }

    #[test]
    fn missing_settings_are_the_defaults_without_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        let (settings, warning) = SettingsStore::new(dir.path()).load();
        assert_eq!(settings, Settings::default());
        assert!(warning.is_none());
    }

    #[test]
    fn corrupt_settings_are_the_defaults_with_a_warning() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        fs::write(store.path(), b"{bad").unwrap();
        let (settings, warning) = store.load();
        assert_eq!(settings, Settings::default());
        assert!(warning.unwrap().contains("settings.json is not valid"));
        // Loading never rewrites the file.
        assert_eq!(fs::read(store.path()).unwrap(), b"{bad");
    }

    #[test]
    fn saving_settings_keeps_keys_bs_does_not_know() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("new"));
        fs::create_dir_all(dir.path().join("new")).unwrap();
        fs::write(store.path(), br#"{"theme":"nord","future":{"x":1}}"#).unwrap();
        let (mut settings, _) = store.load();
        assert_eq!(settings.theme.as_deref(), Some("nord"));
        settings.theme = Some("dracula".into());
        store.save(&settings).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({"theme": "dracula", "future": {"x": 1}})
        );
    }

    #[test]
    fn save_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        store.save(&sample()).unwrap();
        store.save(&sample()).unwrap();
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["session.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn session_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = SessionStore::new(dir.path());
        fs::write(store.path(), b"old").unwrap();
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644)).unwrap();
        store.save(&sample()).unwrap();
        let mode = fs::metadata(store.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
