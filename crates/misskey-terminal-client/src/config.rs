//! Where msky keeps its logins on disk, as bsky keeps its own.
//!
//! The files are in the config directory: `$MSKY_CONFIG_DIR` when set,
//! otherwise `<platform config dir>/msky` (`$XDG_CONFIG_HOME/msky` or
//! `~/.config/msky` on Linux, `~/Library/Application Support/msky` on macOS,
//! `%APPDATA%\msky` on Windows).
//!
//! - `accounts/<host>.json` holds the access token of the account logged in
//!   on that server, so it is written with owner-only permissions on Unix.
//!   One server has one account; logging in there again replaces it.
//! - `accounts.json` names the server of the account in use. No secrets.
//!
//! `msky logout` removes a server's file.

use terminal_kit::paths::write_private;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Environment variable that overrides the config directory.
const CONFIG_DIR_ENV: &str = "MSKY_CONFIG_DIR";

/// Environment variable holding an access token to use instead of the
/// saved logins: one made in the server's settings (Settings, API), for
/// a run with no one at the keyboard, such as a CI job.
const TOKEN_ENV: &str = "MSKY_TOKEN";

/// The token `MSKY_TOKEN` holds, if it holds one.
pub(crate) fn env_token() -> Option<String> {
    std::env::var(TOKEN_ENV)
        .ok()
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())
}

const ACCOUNTS_DIR: &str = "accounts";
const ACCOUNTS_FILE: &str = "accounts.json";

/// An account logged in on one server. Its `Debug` leaves the token out.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Session {
    /// The server's origin, `https://misskey.io`, as `normalize_host` puts it.
    pub service: String,
    /// The account's ID on that server.
    pub user_id: String,
    /// The account's name at login time.
    pub username: String,
    /// The access token MiAuth gave.
    pub token: String,
    /// What the token was given, as `msky login` asked for it. A login
    /// saved before msky could write has none of the `write:` ones.
    #[serde(default)]
    pub permissions: Vec<String>,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("service", &self.service)
            .field("user_id", &self.user_id)
            .field("username", &self.username)
            .field("permissions", &self.permissions)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// The server's host (and port), as an account's `@name@host` is written.
    pub(crate) fn host(&self) -> &str {
        crate::api::authority(&self.service)
    }
}

/// The config directory from the environment.
pub(crate) fn config_dir() -> Result<PathBuf> {
    terminal_kit::paths::resolve_app_dir(
        std::env::var_os(CONFIG_DIR_ENV).as_deref(),
        dirs::config_dir().as_deref(),
        "msky",
    )
    .ok_or_else(|| {
        Error::io("cannot determine the config directory")
            .with_hint(format!("set {CONFIG_DIR_ENV} to a writable directory"))
    })
}

/// What `settings.json` in the config directory sets. Every field is
/// optional, and a key msky does not know is left alone.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(crate) struct Settings {
    /// The color theme, by name (bsky's themes).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// The language the client is shown in: `en`, `ja`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The folder the client saves files in.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
    /// The program links open with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    /// The reactions given lately, the newest first.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recent_reactions: Option<Vec<String>>,
    /// The columns of the Home tab, by the account they are for
    /// (`username@host`), as bsky keeps its own by DID.
    #[serde(
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "known_columns"
    )]
    pub columns: BTreeMap<String, Vec<ColumnSource>>,
    /// Keys a newer version wrote, kept on saving.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
    /// The columns [`Self::columns`] leaves out (a kind a newer version
    /// added), by account, as they were written: saving puts them back.
    #[serde(skip)]
    pub unknown_columns: BTreeMap<String, Vec<serde_json::Value>>,
}

/// The columns of the settings file `data` that this version cannot read,
/// by account.
fn unknown_columns(data: &[u8]) -> BTreeMap<String, Vec<serde_json::Value>> {
    let Ok(serde_json::Value::Object(mut file)) = serde_json::from_slice(data) else {
        return BTreeMap::new();
    };
    let Some(serde_json::Value::Object(columns)) = file.remove("columns") else {
        return BTreeMap::new();
    };
    columns
        .into_iter()
        .filter_map(|(account, list)| {
            let serde_json::Value::Array(list) = list else {
                return None;
            };
            let unknown: Vec<serde_json::Value> = list
                .into_iter()
                .filter(|c| ColumnSource::deserialize(c).is_err())
                .collect();
            (!unknown.is_empty()).then_some((account, unknown))
        })
        .collect()
}

/// The columns of `settings.json`, leaving out any this version cannot
/// read (a kind a newer version added): one column is not worth ignoring
/// the whole file over.
fn known_columns<'de, D>(d: D) -> std::result::Result<BTreeMap<String, Vec<ColumnSource>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: BTreeMap<String, Vec<serde_json::Value>> = Deserialize::deserialize(d)?;
    Ok(raw
        .into_iter()
        .map(|(account, columns)| {
            let known = columns
                .into_iter()
                .filter_map(|c| serde_json::from_value(c).ok())
                .collect();
            (account, known)
        })
        .collect())
}

/// What a column of the Home tab shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ColumnSource {
    /// The home timeline.
    Home,
    /// An antenna, a list or a channel of the account, by ID, and the name
    /// to show.
    Antenna {
        id: String,
        name: String,
    },
    List {
        id: String,
        name: String,
    },
    Channel {
        id: String,
        name: String,
    },
    Notifications,
    /// Notes found for `query`.
    Search {
        query: String,
    },
    /// An account's own notes, by ID, and the name to show.
    Author {
        id: String,
        username: String,
    },
}

impl ColumnSource {
    /// The title of the column.
    pub(crate) fn title(&self) -> String {
        use crate::i18n::{t, tf};
        use crate::tui::text::one_line;
        match self {
            ColumnSource::Home => t("Home").into(),
            ColumnSource::Antenna { name, .. } => format!("📡 {}", one_line(name)),
            ColumnSource::List { name, .. } => format!("☰ {}", one_line(name)),
            ColumnSource::Channel { name, .. } => format!("# {}", one_line(name)),
            ColumnSource::Notifications => t("Notifications").into(),
            ColumnSource::Search { query } => tf("Search: {}", &[query]),
            ColumnSource::Author { username, .. } => format!("@{username}"),
        }
    }
}

const SETTINGS_FILE: &str = "settings.json";

/// The settings in `dir`, and why the file could not be read when it was
/// there and could not (the defaults are used then).
pub(crate) fn load_settings(dir: &Path) -> (Settings, Option<String>) {
    let path = dir.join(SETTINGS_FILE);
    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return (Settings::default(), None),
        Err(e) => {
            return (
                Settings::default(),
                Some(format!("cannot read {}: {e}", path.display())),
            );
        }
    };
    match serde_json::from_slice::<Settings>(without_bom(&data)) {
        Ok(mut s) => {
            s.unknown_columns = unknown_columns(without_bom(&data));
            (s, None)
        }
        Err(e) => (
            Settings::default(),
            Some(format!(
                "{} is not valid settings (line {}, column {}); the defaults are used",
                path.display(),
                e.line(),
                e.column()
            )),
        ),
    }
}

/// Write `settings` to `settings.json` in `dir`, whole or not at all.
pub(crate) fn save_settings(dir: &Path, settings: &Settings) -> Result<()> {
    let path = dir.join(SETTINGS_FILE);
    fs::create_dir_all(dir)
        .map_err(|e| Error::io(format!("cannot create {}: {e}", dir.display())))?;
    let cannot_write =
        |e: &dyn std::fmt::Display| Error::io(format!("cannot write {}: {e}", path.display()));
    let mut value = serde_json::to_value(settings).map_err(|e| cannot_write(&e))?;
    // The columns a newer version wrote go back after the ones this
    // version keeps, for that version to show again.
    if !settings.unknown_columns.is_empty()
        && let Some(file) = value.as_object_mut()
    {
        let columns = file
            .entry("columns")
            .or_insert_with(|| serde_json::Value::Object(Default::default()));
        if let Some(columns) = columns.as_object_mut() {
            for (account, unknown) in &settings.unknown_columns {
                let list = columns
                    .entry(account.clone())
                    .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                if let Some(list) = list.as_array_mut() {
                    list.extend(unknown.iter().cloned());
                }
            }
        }
    }
    let mut json = serde_json::to_vec_pretty(&value).map_err(|e| cannot_write(&e))?;
    json.push(b'\n');
    write_private(&path, &json).map_err(|e| cannot_write(&e))
}

/// Which server's account is in use, kept in `accounts.json`.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Current {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current: Option<String>,
    /// Keys a newer version wrote, kept on saving.
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

/// The accounts logged in from one config directory: a file per server
/// under `accounts/`, and `accounts.json` naming the one in use.
#[derive(Debug, Clone)]
pub(crate) struct AccountStore {
    dir: PathBuf,
}

impl AccountStore {
    /// The accounts of the config directory `dir`; nothing is touched
    /// until a read or write.
    pub(crate) fn open(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn accounts_dir(&self) -> PathBuf {
        self.dir.join(ACCOUNTS_DIR)
    }

    /// The config directory the accounts are kept in.
    pub(crate) fn dir(&self) -> &Path {
        &self.dir
    }

    /// The file of the account on `host`.
    fn path_for(&self, host: &str) -> PathBuf {
        self.accounts_dir()
            .join(format!("{}.json", file_stem(&host.to_ascii_lowercase())))
    }

    /// The account logged in on `host`, or `None`.
    pub(crate) fn find(&self, host: &str) -> Result<Option<Session>> {
        load(&self.path_for(host))
    }

    /// Every account, by host.
    pub(crate) fn list(&self) -> Result<Vec<Session>> {
        let mut all = Vec::new();
        for path in self.account_files()? {
            if let Some(s) = load(&path)? {
                all.push(s);
            }
        }
        all.sort_by(|a, b| a.host().cmp(b.host()));
        Ok(all)
    }

    /// The account in use: the one `accounts.json` names, else the first.
    pub(crate) fn current(&self) -> Result<Option<Session>> {
        if let Some(host) = self.read_current().current
            && let Some(s) = self.find(&host)?
        {
            return Ok(Some(s));
        }
        Ok(self.list()?.into_iter().next())
    }

    /// Save an account, making it the one in use.
    pub(crate) fn save(&self, session: &Session) -> Result<()> {
        let dir = self.accounts_dir();
        fs::create_dir_all(&dir)
            .map_err(|e| Error::io(format!("cannot create {}: {e}", dir.display())))?;
        let path = self.path_for(session.host());
        let mut json = serde_json::to_vec_pretty(session)
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))?;
        json.push(b'\n');
        write_private(&path, &json)
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))?;
        self.write_current(Some(session.host()))
    }

    /// Make the account on `host` the one in use, as `msky accounts --use`
    /// and the client's account list do.
    pub(crate) fn set_current(&self, host: &str) -> Result<()> {
        self.write_current(Some(&host.to_ascii_lowercase()))
    }

    /// The host `accounts.json` names as the one in use, without reading
    /// its account's file.
    pub(crate) fn current_host(&self) -> Option<String> {
        self.read_current().current
    }

    /// Log the account on `host` out: its file goes, one that cannot be
    /// read too. When it was the one in use, the next one by host is.
    /// Returns whether there was a file.
    pub(crate) fn remove(&self, host: &str) -> Result<bool> {
        let existed = remove_file(&self.path_for(host))?;
        let current = self.read_current().current;
        if current
            .as_deref()
            .is_some_and(|c| c.eq_ignore_ascii_case(host))
        {
            let next = self.list()?.into_iter().next();
            self.write_current(next.as_ref().map(Session::host))?;
        }
        Ok(existed)
    }

    /// Log every account out, one whose file cannot be read too (that is
    /// the way out of it). Each is returned with its host, by host; one that
    /// could not be read is named by its file.
    pub(crate) fn remove_all(&self) -> Result<Vec<(Option<Session>, String)>> {
        let mut gone = Vec::new();
        for path in self.account_files()? {
            let was = load(&path).ok().flatten();
            let host = match &was {
                Some(s) => s.host().to_string(),
                None => path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            };
            remove_file(&path)?;
            gone.push((was, host));
        }
        if self.dir.join(ACCOUNTS_FILE).exists() {
            self.write_current(None)?;
        }
        gone.sort_by(|a, b| a.1.cmp(&b.1));
        Ok(gone)
    }

    /// The file of every account; none when there is no accounts folder yet.
    fn account_files(&self) -> Result<Vec<PathBuf>> {
        let dir = self.accounts_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(Error::io(format!("cannot read {}: {e}", dir.display()))),
        };
        let mut files: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        files.sort();
        Ok(files)
    }

    fn read_current(&self) -> Current {
        fs::read(self.dir.join(ACCOUNTS_FILE))
            .ok()
            .and_then(|d| serde_json::from_slice(without_bom(&d)).ok())
            .unwrap_or_default()
    }

    fn write_current(&self, host: Option<&str>) -> Result<()> {
        let mut c = self.read_current();
        c.current = host.map(str::to_string);
        fs::create_dir_all(&self.dir)
            .map_err(|e| Error::io(format!("cannot create {}: {e}", self.dir.display())))?;
        let path = self.dir.join(ACCOUNTS_FILE);
        let mut json = serde_json::to_vec_pretty(&c)
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))?;
        json.push(b'\n');
        write_private(&path, &json)
            .map_err(|e| Error::io(format!("cannot write {}: {e}", path.display())))
    }
}

/// The account saved in `path`; `Ok(None)` when there is none.
fn load(path: &Path) -> Result<Option<Session>> {
    let data = match fs::read(path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(Error::io(format!("cannot read {}: {e}", path.display()))),
    };
    // The parser's message can quote the file, token and all: only where it
    // went wrong is said.
    serde_json::from_slice(without_bom(&data))
        .map(Some)
        .map_err(|e| {
            Error::io(format!(
                "{} is not a valid login file (line {}, column {})",
                path.display(),
                e.line(),
                e.column()
            ))
            .with_hint("run `msky logout --all` to discard it and log in again")
        })
}

/// Remove `path`; whether it was there.
fn remove_file(path: &Path) -> Result<bool> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(Error::io(format!("cannot remove {}: {e}", path.display()))),
    }
}

fn without_bom(data: &[u8]) -> &[u8] {
    data.strip_prefix(b"\xef\xbb\xbf").unwrap_or(data)
}

/// A host as a file name on every system: `127.0.0.1:8080` has a colon,
/// which Windows does not allow.
fn file_stem(host: &str) -> String {
    host.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway value in the place of a token; no server gave it.
    const FAKE: &str = "fake-token-for-tests";

    fn account(service: &str, name: &str) -> Session {
        Session {
            service: service.into(),
            user_id: format!("id-{name}"),
            username: name.into(),
            token: format!("{FAKE}-{name}"),
            permissions: vec!["read:account".into()],
        }
    }

    // Printed with {:?}, a session must not show its token.
    #[test]
    fn a_session_printed_for_debugging_hides_its_token() {
        let shown = format!("{:?}", account("https://misskey.test", "alice"));
        assert!(!shown.contains(FAKE), "the token is hidden");
        assert!(shown.contains("alice"), "the username is shown");
    }

    #[test]
    fn nothing_saved_is_no_account() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path().join("none"));
        assert_eq!(store.find("misskey.io").unwrap(), None);
        assert_eq!(store.current().unwrap(), None);
        assert!(store.list().unwrap().is_empty());
        assert!(!dir.path().join("none").exists(), "reading writes nothing");
    }

    // One file per server, named after its host and port; the last saved
    // is the one in use, and saving on a server again replaces its account.
    #[test]
    fn each_server_has_a_file_and_the_last_saved_is_in_use() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("https://misskey.io", "alice")).unwrap();
        store
            .save(&account("http://127.0.0.1:8080", "bob"))
            .unwrap();
        assert!(dir.path().join("accounts/misskey.io.json").exists());
        assert!(dir.path().join("accounts/127.0.0.1_8080.json").exists());
        assert_eq!(store.current().unwrap().unwrap().username, "bob");
        assert_eq!(store.find("MISSKEY.IO").unwrap().unwrap().username, "alice");
        store.save(&account("https://misskey.io", "carol")).unwrap();
        assert_eq!(store.current().unwrap().unwrap().username, "carol");
        let hosts: Vec<String> = store
            .list()
            .unwrap()
            .iter()
            .map(|s| s.host().to_string())
            .collect();
        assert_eq!(hosts, ["127.0.0.1:8080", "misskey.io"]);
    }

    // Logging the account in use out moves on to the next; logging out an
    // account that is not there says so; --all takes a broken file too.
    #[test]
    fn logging_out_removes_the_file_and_moves_on() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("https://a.test", "alice")).unwrap();
        store.save(&account("https://b.test", "bob")).unwrap();
        assert_eq!(store.current_host().as_deref(), Some("b.test"));
        assert!(store.remove("b.test").unwrap());
        assert!(!dir.path().join("accounts/b.test.json").exists());
        assert_eq!(store.current().unwrap().unwrap().username, "alice");
        assert_eq!(store.current_host().as_deref(), Some("a.test"));
        assert!(!store.remove("b.test").unwrap());
        fs::write(dir.path().join("accounts/c.test.json"), "{broken").unwrap();
        let all = store.remove_all().unwrap();
        let named: Vec<(bool, &str)> = all.iter().map(|(s, h)| (s.is_some(), h.as_str())).collect();
        assert_eq!(named, [(true, "a.test"), (false, "c.test")]);
        assert!(store.list().unwrap().is_empty());
        assert_eq!(store.current().unwrap(), None);
    }

    // A broken file says where it is and how to get out of it, without
    // quoting what is in it: a token could be there.
    #[test]
    fn a_broken_file_is_an_error_that_does_not_quote_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        fs::create_dir_all(dir.path().join("accounts")).unwrap();
        fs::write(
            dir.path().join("accounts/misskey.io.json"),
            format!("{{\"token\": \"{FAKE}\" oops"),
        )
        .unwrap();
        let e = store.find("misskey.io").unwrap_err();
        assert_eq!(e.kind(), crate::error::Kind::Io);
        assert!(!e.to_string().contains(FAKE), "{e}");
        assert!(e.hint().unwrap().contains("msky logout --all"));
    }

    #[test]
    fn a_saved_account_reads_back_with_a_byte_order_mark_too() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        let a = account("https://misskey.io", "alice");
        store.save(&a).unwrap();
        let path = store.path_for("misskey.io");
        let mut data = b"\xef\xbb\xbf".to_vec();
        data.extend(fs::read(&path).unwrap());
        fs::write(&path, data).unwrap();
        assert_eq!(store.find("misskey.io").unwrap(), Some(a));
    }

    #[test]
    fn saving_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("https://misskey.io", "alice")).unwrap();
        let names: Vec<String> = fs::read_dir(dir.path().join("accounts"))
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["misskey.io.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn the_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        let path = store.path_for("misskey.io");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        // A file left with a loose mode is tightened when it is written.
        fs::write(&path, "{}").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        store.save(&account("https://misskey.io", "alice")).unwrap();
        for p in [path, dir.path().join(ACCOUNTS_FILE)] {
            let mode = fs::metadata(&p).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600, "{}", p.display());
        }
    }

    #[test]
    fn settings_are_read_and_a_broken_file_says_where() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(load_settings(dir.path()), (Settings::default(), None));
        fs::write(
            dir.path().join(SETTINGS_FILE),
            "\u{feff}{\"tabs\": [\"home\", \"social\"], \"later\": 1}",
        )
        .unwrap();
        let (s, warn) = load_settings(dir.path());
        assert_eq!(warn, None);
        // The tabs setting of an earlier version is not read, and not an
        // error; saved, it stays, as everything this version does not know.
        assert_eq!(s.other["tabs"], serde_json::json!(["home", "social"]));
        let mut s2 = s.clone();
        s2.theme = Some("dracula".into());
        save_settings(dir.path(), &s2).unwrap();
        let v: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join(SETTINGS_FILE)).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"tabs": ["home", "social"], "theme": "dracula", "later": 1})
        );
        fs::write(dir.path().join(SETTINGS_FILE), "{\"tabs\": oops").unwrap();
        let (s, warn) = load_settings(dir.path());
        assert_eq!(s, Settings::default());
        assert!(warn.unwrap().contains("is not valid settings (line 1"));
    }

    // A column of a kind a newer version added is left out of what this one
    // shows; the rest of the file is still read, and saving puts the column
    // back after the ones this version keeps.
    #[test]
    fn a_column_of_an_unknown_kind_is_left_out_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS_FILE),
            serde_json::to_vec(
                &serde_json::json!({"theme": "nord", "columns": {"me@misskey.test": [
                    {"kind": "home"},
                    {"kind": "gallery", "id": "g1"},
                    {"kind": "search", "query": "猫 🐈‍⬛"}
                ]}}),
            )
            .unwrap(),
        )
        .unwrap();
        let (mut s, warn) = load_settings(dir.path());
        assert_eq!(warn, None);
        assert_eq!(s.theme.as_deref(), Some("nord"));
        assert_eq!(
            s.columns["me@misskey.test"],
            [
                ColumnSource::Home,
                ColumnSource::Search {
                    query: "猫 🐈‍⬛".into()
                }
            ]
        );
        s.columns
            .get_mut("me@misskey.test")
            .unwrap()
            .push(ColumnSource::Notifications);
        save_settings(dir.path(), &s).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join(SETTINGS_FILE)).unwrap()).unwrap();
        assert_eq!(
            saved["columns"]["me@misskey.test"],
            serde_json::json!([
                {"kind": "home"},
                {"kind": "search", "query": "猫 🐈‍⬛"},
                {"kind": "notifications"},
                {"kind": "gallery", "id": "g1"}
            ])
        );
        assert!(saved.get("unknownColumns").is_none());
    }

    #[test]
    fn keys_a_newer_version_wrote_in_accounts_json_are_kept() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(ACCOUNTS_FILE),
            r#"{"current": "x.test", "later": [1]}"#,
        )
        .unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("https://misskey.io", "alice")).unwrap();
        let v: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join(ACCOUNTS_FILE)).unwrap()).unwrap();
        assert_eq!(
            v,
            serde_json::json!({"current": "misskey.io", "later": [1]})
        );
    }
}
