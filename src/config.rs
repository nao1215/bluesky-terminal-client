//! Where bsky keeps its state on disk: the login session and the settings.
//!
//! Both files live in the config directory: `$BSKY_CONFIG_DIR` when set,
//! otherwise `<platform config dir>/bsky` (`$XDG_CONFIG_HOME/bsky` or
//! `~/.config/bsky` on Linux, `~/Library/Application Support/bsky` on macOS,
//! `%APPDATA%\bsky` on Windows).
//!
//! - `accounts/<did>.json` holds the tokens of each account logged in, so
//!   it is written with owner-only permissions on Unix, and `accounts.json`
//!   names the one in use. `bsky logout` removes an account's file.
//! - `settings.json` holds preferences (the color theme, pictures on or
//!   off, and the folders, video service and browser of the settings
//!   screen). It is written only when a preference is changed, and a broken
//!   one is ignored with a warning rather than stopping bsky.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Environment variable that overrides the config directory.
pub const CONFIG_DIR_ENV: &str = "BSKY_CONFIG_DIR";

const SETTINGS_FILE: &str = "settings.json";

/// User preferences.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    /// Name of the color theme.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub theme: Option<String>,
    /// Whether pictures and video are drawn: `"auto"` (when the terminal
    /// can) or `"off"`. `BSKY_GRAPHICS` in the environment wins over it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pictures: Option<String>,
    /// Where `d` saves; `BSKY_DOWNLOAD_DIR` wins over it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub download_dir: Option<String>,
    /// Where pictures are cached, or `"off"`; `BSKY_CACHE_DIR` wins over it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_dir: Option<String>,
    /// The video service uploads go to; `BSKY_VIDEO_SERVICE` wins over it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub video_service: Option<String>,
    /// The program that opens links; `BSKY_BROWSER` wins over it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    /// The language the client is shown in (`en`, `ja`...); without one,
    /// the language the environment asks for.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The columns of the Timeline tab, by the DID of the account they are
    /// for.
    #[serde(
        default,
        skip_serializing_if = "std::collections::BTreeMap::is_empty",
        deserialize_with = "known_columns"
    )]
    pub columns: std::collections::BTreeMap<String, Vec<ColumnSource>>,
    /// Keys this version of bsky does not know, kept so that saving does not
    /// drop what a newer version wrote.
    #[serde(flatten)]
    pub other: serde_json::Map<String, serde_json::Value>,
    /// The columns [`Self::columns`] leaves out (a kind a newer version
    /// added), by account, as they were written: saving puts them back.
    #[serde(skip)]
    pub unknown_columns: std::collections::BTreeMap<String, Vec<serde_json::Value>>,
}

/// The columns of the settings file `data` that this version cannot read,
/// by account.
fn unknown_columns(data: &[u8]) -> std::collections::BTreeMap<String, Vec<serde_json::Value>> {
    let Ok(serde_json::Value::Object(mut file)) = serde_json::from_slice(data) else {
        return Default::default();
    };
    let Some(serde_json::Value::Object(columns)) = file.remove("columns") else {
        return Default::default();
    };
    columns
        .into_iter()
        .filter_map(|(did, list)| {
            let serde_json::Value::Array(list) = list else {
                return None;
            };
            let unknown: Vec<serde_json::Value> = list
                .into_iter()
                .filter(|c| ColumnSource::deserialize(c).is_err())
                .collect();
            (!unknown.is_empty()).then_some((did, unknown))
        })
        .collect()
}

/// The columns of `settings.json`, leaving out any this version cannot
/// read (a kind a newer version added): one column is not worth ignoring
/// the whole file over.
fn known_columns<'de, D>(
    d: D,
) -> std::result::Result<std::collections::BTreeMap<String, Vec<ColumnSource>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let raw: std::collections::BTreeMap<String, Vec<serde_json::Value>> =
        Deserialize::deserialize(d)?;
    Ok(raw
        .into_iter()
        .map(|(did, columns)| {
            let known = columns
                .into_iter()
                .filter_map(|c| serde_json::from_value(c).ok())
                .collect();
            (did, known)
        })
        .collect())
}

impl Settings {
    /// Whether the file turns pictures off.
    pub fn pictures_off(&self) -> bool {
        self.pictures.as_deref() == Some("off")
    }
}

/// What a column shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ColumnSource {
    Following,
    /// A custom feed by the URI of its generator, and the name to show.
    Feed {
        uri: String,
        name: String,
    },
    Notifications,
    /// Posts found for `query`.
    Search {
        query: String,
    },
    /// An account's own posts, by DID, and the handle to show.
    Author {
        did: String,
        handle: String,
    },
}

impl ColumnSource {
    /// The title of the column.
    pub fn title(&self) -> String {
        use crate::i18n::{t, tf};
        match self {
            ColumnSource::Following => t("Following").into(),
            ColumnSource::Feed { name, .. } => name.clone(),
            ColumnSource::Notifications => t("Notifications").into(),
            ColumnSource::Search { query } => tf("Search: {}", &[query]),
            ColumnSource::Author { handle, .. } => format!("@{handle}"),
        }
    }
}

/// The variables that fix a setting for this run, read once at the start.
/// A variable set here wins over `settings.json`, and the settings screen
/// says so rather than fighting it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    /// `BSKY_GRAPHICS`
    pub graphics: Option<String>,
    /// `BSKY_DOWNLOAD_DIR`
    pub download_dir: Option<String>,
    /// `BSKY_CACHE_DIR`
    pub cache_dir: Option<String>,
    /// `BSKY_VIDEO_SERVICE`
    pub video_service: Option<String>,
    /// `BSKY_BROWSER`
    pub browser: Option<String>,
    /// The locale the environment asks for: `LC_ALL`, `LC_MESSAGES` or
    /// `LANG`, the first one set, as every program reads them.
    pub locale: Option<String>,
}

impl Environment {
    /// The variables as this process sees them; an empty one is not set.
    pub fn read() -> Self {
        Self::from_vars(|k| std::env::var(k).ok())
    }

    fn from_vars(get: impl Fn(&str) -> Option<String>) -> Self {
        let get = |k: &str| get(k).filter(|v| !v.trim().is_empty());
        Self {
            graphics: get(crate::terminal::GRAPHICS_ENV),
            download_dir: get(DOWNLOAD_DIR_ENV),
            cache_dir: get(CACHE_DIR_ENV),
            video_service: get(VIDEO_SERVICE_ENV),
            browser: get(crate::browser::BROWSER_ENV),
            locale: ["LC_ALL", "LC_MESSAGES", "LANG"].into_iter().find_map(get),
        }
    }
}

/// A file's bytes without the byte order mark some editors put at the start
/// of UTF-8 (Notepad did), which JSON does not allow.
fn without_bom(data: &[u8]) -> &[u8] {
    data.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(data)
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
            Ok(data) => match serde_json::from_slice::<Settings>(without_bom(&data)) {
                Ok(mut settings) => {
                    settings.unknown_columns = unknown_columns(without_bom(&data));
                    (settings, None)
                }
                Err(e) => (
                    Settings::default(),
                    Some(crate::i18n::tf(
                        "{} is not valid and was ignored: {}",
                        &[&(path.display()).to_string(), &e.to_string()],
                    )),
                ),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (Settings::default(), None),
            Err(e) => (
                Settings::default(),
                Some(crate::i18n::tf(
                    "cannot read {}: {}",
                    &[&(path.display()).to_string(), &e.to_string()],
                )),
            ),
        }
    }

    /// Save the settings, creating the directory when needed.
    pub fn save(&self, settings: &Settings) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(|e| {
            Error::io(crate::i18n::tf(
                "cannot create {}: {}",
                &[&(self.dir.display()).to_string(), &e.to_string()],
            ))
        })?;
        let path = self.path();
        let mut value = serde_json::to_value(settings).expect("settings serialize");
        // The columns a newer version wrote go back after the ones this
        // version keeps, for that version to show again.
        if !settings.unknown_columns.is_empty()
            && let Some(file) = value.as_object_mut()
        {
            let columns = file
                .entry("columns")
                .or_insert_with(|| serde_json::Value::Object(Default::default()));
            if let Some(columns) = columns.as_object_mut() {
                for (did, unknown) in &settings.unknown_columns {
                    let list = columns
                        .entry(did.clone())
                        .or_insert_with(|| serde_json::Value::Array(Vec::new()));
                    if let Some(list) = list.as_array_mut() {
                        list.extend(unknown.iter().cloned());
                    }
                }
            }
        }
        let mut json = serde_json::to_vec_pretty(&value).expect("settings serialize");
        json.push(b'\n');
        write_private(&path, &json).map_err(|e| {
            Error::io(crate::i18n::tf(
                "cannot write {}: {}",
                &[&(path.display()).to_string(), &e.to_string()],
            ))
        })
    }
}

/// An authenticated session against one PDS. Its `Debug` leaves the tokens
/// out.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("service", &self.service)
            .field("did", &self.did)
            .field("handle", &self.handle)
            .finish_non_exhaustive()
    }
}

/// Resolve the config directory from the environment.
pub fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    dirs::config_dir().map(|d| d.join("bsky")).ok_or_else(|| {
        Error::io(crate::i18n::t("cannot determine the config directory")).with_hint(
            crate::i18n::tf("set {} to a writable directory", &[CONFIG_DIR_ENV]),
        )
    })
}

/// Where a setting's value comes from, in the order they win.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A `BSKY_` variable, for this run.
    Env,
    /// `settings.json`.
    File,
    /// Nothing named one.
    Default,
}

/// The variable when it is set, else the file's value, each with where it
/// came from. The one place the order is decided, so the settings screen and
/// what bsky does cannot disagree.
fn pick<'a>(env: &'a Option<String>, file: &'a Option<String>) -> Option<(&'a str, Source)> {
    let set = |v: &'a Option<String>| v.as_deref().map(str::trim).filter(|v| !v.is_empty());
    set(env)
        .map(|v| (v, Source::Env))
        .or_else(|| set(file).map(|v| (v, Source::File)))
}

/// Environment variable naming the folder downloads are saved in.
pub const DOWNLOAD_DIR_ENV: &str = "BSKY_DOWNLOAD_DIR";

/// Where the viewer's `d` saves pictures and videos: `BSKY_DOWNLOAD_DIR`,
/// else the settings, else `bsky` in the platform download folder
/// (`~/Downloads/bsky`).
pub fn download_dir(env: &Environment, settings: &Settings) -> (Option<PathBuf>, Source) {
    match pick(&env.download_dir, &settings.download_dir) {
        Some((v, from)) => (Some(PathBuf::from(v)), from),
        None => (default_download_dir(), Source::Default),
    }
}

/// The download folder when nothing names another: `bsky` in the platform
/// download folder.
pub fn default_download_dir() -> Option<PathBuf> {
    dirs::download_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Downloads")))
        .map(|d| d.join("bsky"))
}

/// Environment variable naming the video service to upload videos to.
pub const VIDEO_SERVICE_ENV: &str = "BSKY_VIDEO_SERVICE";

/// The language `settings` name, else the one the environment asks for,
/// else English.
pub fn language(settings: &Settings, env: &Environment) -> crate::i18n::Lang {
    use crate::i18n::Lang;
    settings
        .language
        .as_deref()
        .and_then(Lang::from_code)
        .or_else(|| env.locale.as_deref().and_then(Lang::from_code))
        .unwrap_or(Lang::En)
}

/// The video service: `BSKY_VIDEO_SERVICE`, else the settings, else
/// Bluesky's.
pub fn video_service(env: &Environment, settings: &Settings) -> (String, Source) {
    match pick(&env.video_service, &settings.video_service) {
        Some((v, from)) => (v.trim_end_matches('/').to_string(), from),
        None => (
            crate::api::DEFAULT_VIDEO_SERVICE.to_string(),
            Source::Default,
        ),
    }
}

/// Environment variable naming the cache directory; `off` keeps no cache.
pub const CACHE_DIR_ENV: &str = "BSKY_CACHE_DIR";

/// Where downloaded pictures are kept between runs, or `None` for no cache:
/// `BSKY_CACHE_DIR`, else the settings (`off` in either keeps none), else
/// `bsky` in the platform cache folder.
pub fn cache_dir(env: &Environment, settings: &Settings) -> (Option<PathBuf>, Source) {
    cache_dir_from(env, settings, dirs::cache_dir())
}

fn cache_dir_from(
    env: &Environment,
    settings: &Settings,
    platform: Option<PathBuf>,
) -> (Option<PathBuf>, Source) {
    match pick(&env.cache_dir, &settings.cache_dir) {
        Some(("off", from)) => (None, from),
        Some((v, from)) => (Some(PathBuf::from(v)), from),
        None => (platform.map(|d| d.join("bsky")), Source::Default),
    }
}

/// The program that opens links: `BSKY_BROWSER`, else the settings, else
/// `None` for the system's own (see [`crate::browser`]).
pub fn browser(env: &Environment, settings: &Settings) -> (Option<String>, Source) {
    match pick(&env.browser, &settings.browser) {
        Some((v, from)) => (Some(v.to_string()), from),
        None => (None, Source::Default),
    }
}

/// Whether bsky can write into `dir`, creating it when it is not there yet:
/// a folder chosen for downloads or the cache is tried at once, so a bad one
/// is refused with the reason rather than at the next download.
pub fn check_writable(dir: &Path) -> std::result::Result<(), String> {
    let fail = |e: std::io::Error| {
        crate::i18n::tf(
            "cannot write to {}: {}",
            &[&(dir.display()).to_string(), &e.to_string()],
        )
    };
    fs::create_dir_all(dir).map_err(fail)?;
    // A new file only, never through a link or over a file left at a name:
    // a name that is taken is passed over for the next.
    for n in 0..100 {
        let probe = dir.join(format!(".bsky-write-test-{}-{n}", std::process::id()));
        match fs::File::options()
            .write(true)
            .create_new(true)
            .open(&probe)
        {
            Ok(_) => {
                let _ = fs::remove_file(&probe);
                return Ok(());
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(fail(e)),
        }
    }
    Err(crate::i18n::tf(
        "cannot write to {}: every name tried was taken",
        &[&(dir.display()).to_string()],
    ))
}

/// Reads and writes the session file inside one directory.
#[derive(Debug, Clone)]
pub struct SessionStore {
    dir: PathBuf,
    file: String,
}

impl SessionStore {
    /// Path of the session file.
    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.file)
    }

    /// Load the saved session; `Ok(None)` when there is none yet.
    pub fn load(&self) -> Result<Option<Session>> {
        let path = self.path();
        let data = match fs::read(&path) {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(Error::io(crate::i18n::tf(
                    "cannot read {}: {}",
                    &[&(path.display()).to_string(), &e.to_string()],
                )));
            }
        };
        serde_json::from_slice(without_bom(&data))
            .map(Some)
            .map_err(|e| {
                Error::io(crate::i18n::tf(
                    "{} is not a valid session file: {}",
                    &[&(path.display()).to_string(), &e.to_string()],
                ))
                .with_hint(crate::i18n::t(
                    "run `bsky logout --all` to discard it and log in again",
                ))
            })
    }

    /// Save refreshed tokens, only while the file is there: an account
    /// logged out while a load of it was on its way stays logged out.
    pub fn update(&self, session: &Session) -> Result<()> {
        if !self.path().exists() {
            return Ok(());
        }
        self.save(session)
    }

    /// Save the session, creating the directory when needed.
    pub fn save(&self, session: &Session) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(|e| {
            Error::io(crate::i18n::tf(
                "cannot create {}: {}",
                &[&(self.dir.display()).to_string(), &e.to_string()],
            ))
        })?;
        let path = self.path();
        let json = serde_json::to_vec_pretty(session).expect("session serializes");
        write_private(&path, &json).map_err(|e| {
            Error::io(crate::i18n::tf(
                "cannot write {}: {}",
                &[&(path.display()).to_string(), &e.to_string()],
            ))
        })
    }

    /// Remove the saved session. Returns whether a session existed.
    pub fn clear(&self) -> Result<bool> {
        let path = self.path();
        match fs::remove_file(&path) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(Error::io(crate::i18n::tf(
                "cannot remove {}: {}",
                &[&(path.display()).to_string(), &e.to_string()],
            ))),
        }
    }
}

const ACCOUNTS_DIR: &str = "accounts";
const ACCOUNTS_FILE: &str = "accounts.json";

/// Which account is in use, kept in `accounts.json`. No secrets.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Current {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    current: Option<String>,
    /// Keys a newer version wrote, kept on saving.
    #[serde(flatten)]
    other: serde_json::Map<String, serde_json::Value>,
}

/// The logged-in accounts of one config directory: a session file per
/// account under `accounts/`, and `accounts.json` naming the one in use.
///
/// Each account's tokens are in a file of their own, so a refresh of one
/// rewrites that file alone: two accounts refreshing at once cannot lose
/// each other's rotated tokens.
#[derive(Debug, Clone)]
pub struct AccountStore {
    dir: PathBuf,
}

impl AccountStore {
    /// The accounts of the config directory `dir`; nothing is touched
    /// until a read or write.
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn accounts_dir(&self) -> PathBuf {
        self.dir.join(ACCOUNTS_DIR)
    }

    /// The session file of the account `did`, for a client to save its
    /// refreshed tokens to.
    pub fn store_for(&self, did: &str) -> SessionStore {
        SessionStore {
            dir: self.accounts_dir(),
            file: format!("{}.json", file_stem(did)),
        }
    }

    /// The session file of every account; none when there is no accounts
    /// folder yet.
    fn account_files(&self) -> Result<Vec<SessionStore>> {
        let dir = self.accounts_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => {
                return Err(Error::io(crate::i18n::tf(
                    "cannot read {}: {}",
                    &[&(dir.display()).to_string(), &e.to_string()],
                )));
            }
        };
        Ok(entries
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                name.ends_with(".json").then(|| SessionStore {
                    dir: dir.clone(),
                    file: name,
                })
            })
            .collect())
    }

    /// Log every account out: each account file goes, one that cannot be
    /// read too (that is the way out of it). The accounts that could be read
    /// are returned, to say who was logged out.
    pub fn remove_all(&self) -> Result<Vec<Session>> {
        let mut gone = Vec::new();
        for store in self.account_files()? {
            if let Ok(Some(s)) = store.load() {
                gone.push(s);
            }
            store.clear()?;
        }
        self.write_current(None)?;
        gone.sort_by_key(|a| a.handle.to_lowercase());
        Ok(gone)
    }

    /// Every account, by handle.
    pub fn list(&self) -> Result<Vec<Session>> {
        let mut all = Vec::new();
        for store in self.account_files()? {
            if let Some(s) = store.load()? {
                all.push(s);
            }
        }
        all.sort_by(|a, b| {
            (a.handle.to_lowercase(), &a.did).cmp(&(b.handle.to_lowercase(), &b.did))
        });
        Ok(all)
    }

    /// The account in use: the one `accounts.json` names, else the first.
    pub fn current(&self) -> Result<Option<Session>> {
        let all = self.list()?;
        let named = self.read_current().current;
        Ok(named
            .and_then(|did| all.iter().find(|s| s.did == did).cloned())
            .or_else(|| all.into_iter().next()))
    }

    /// The account `who` names: its DID, its handle, or `@handle`, in any
    /// case.
    pub fn find(&self, who: &str) -> Result<Option<Session>> {
        let who = who.trim().trim_start_matches('@');
        Ok(self
            .list()?
            .into_iter()
            .find(|s| s.did == who || s.handle.eq_ignore_ascii_case(who)))
    }

    /// Make `did` the account in use.
    pub fn set_current(&self, did: &str) -> Result<()> {
        self.write_current(Some(did))
    }

    /// Save an account's session, making it the one in use.
    pub fn save(&self, session: &Session) -> Result<()> {
        self.store_for(&session.did).save(session)?;
        self.set_current(&session.did)
    }

    /// Log the account `did` out: its session file goes. When it was the
    /// one in use, the next one by handle is. Returns whether it existed.
    pub fn remove(&self, did: &str) -> Result<bool> {
        let existed = self.store_for(did).clear()?;
        if self.read_current().current.as_deref() == Some(did) {
            let next = self.list()?.into_iter().next().map(|s| s.did);
            self.write_current(next.as_deref())?;
        }
        Ok(existed)
    }

    fn read_current(&self) -> Current {
        fs::read(self.dir.join(ACCOUNTS_FILE))
            .ok()
            .and_then(|d| serde_json::from_slice(without_bom(&d)).ok())
            .unwrap_or_default()
    }

    fn write_current(&self, did: Option<&str>) -> Result<()> {
        let mut c = self.read_current();
        c.current = did.map(str::to_string);
        fs::create_dir_all(&self.dir).map_err(|e| {
            Error::io(crate::i18n::tf(
                "cannot create {}: {}",
                &[&(self.dir.display()).to_string(), &e.to_string()],
            ))
        })?;
        let path = self.dir.join(ACCOUNTS_FILE);
        let mut json = serde_json::to_vec_pretty(&c).expect("accounts serialize");
        json.push(b'\n');
        write_private(&path, &json).map_err(|e| {
            Error::io(crate::i18n::tf(
                "cannot write {}: {}",
                &[&(path.display()).to_string(), &e.to_string()],
            ))
        })
    }
}

/// A DID as a file name on every system: `did:plc:abc` has colons, which
/// Windows does not allow, and `did:web` may have `%`.
fn file_stem(did: &str) -> String {
    did.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Write `data` to `path` so that a crash leaves either the old file or the
/// new one, never a truncated one: the refresh token is rotated on every
/// refresh, and losing the new one means logging in again.
fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    // The process id keeps two running copies of bsky from writing the same
    // temporary file, which on Windows would make one of the renames fail.
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut file = open_private(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    };
    write().inspect_err(|_| {
        // The temporary file holds the tokens; it must not be left behind.
        let _ = fs::remove_file(&tmp);
    })
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

    // Printed with {:?}, a session showed both tokens.
    #[test]
    fn a_session_printed_for_debugging_hides_its_tokens() {
        let s = Session {
            service: "https://bsky.social".into(),
            did: "did:plc:a".into(),
            handle: "alice.test".into(),
            access_jwt: "access-secret".into(),
            refresh_jwt: "refresh-secret".into(),
        };
        let shown = format!("{s:?}");
        assert!(!shown.contains("secret"), "{shown}");
        assert!(
            shown.contains("did:plc:a") && shown.contains("alice.test"),
            "{shown}"
        );
    }

    // A column of a kind a newer version added is left out; the rest of the
    // file, the other columns included, is still read.
    #[test]
    fn a_column_of_an_unknown_kind_leaves_the_rest_of_the_settings() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"theme": "nord", "columns": {"did:plc:me": [
                {"kind": "following"},
                {"kind": "list", "uri": "at://did:plc:me/app.bsky.graph.list/1"},
                {"kind": "feed", "uri": "at://x"},
                {"kind": "search", "query": "猫🐈‍⬛"}
            ]}}"#,
        )
        .unwrap();
        let (settings, warning) = SettingsStore::new(dir.path()).load();
        assert_eq!(warning, None);
        assert_eq!(settings.theme.as_deref(), Some("nord"));
        assert_eq!(
            settings.columns["did:plc:me"],
            [
                ColumnSource::Following,
                ColumnSource::Search {
                    query: "猫🐈‍⬛".into()
                }
            ]
        );
    }

    // A column a newer version wrote is left out of what this one shows,
    // but not out of the file: saving another setting keeps it, as it keeps
    // the keys this version does not know.
    #[test]
    fn saving_keeps_a_column_of_an_unknown_kind() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        let list =
            serde_json::json!({"kind": "list", "uri": "at://did:plc:me/app.bsky.graph.list/1"});
        fs::write(
            store.path(),
            serde_json::to_vec(&serde_json::json!({"columns": {
                "did:plc:me": [{"kind": "following"}, list],
                "did:plc:work": [list],
            }}))
            .unwrap(),
        )
        .unwrap();
        let (mut settings, _) = store.load();
        assert_eq!(settings.columns["did:plc:me"], [ColumnSource::Following]);
        settings.theme = Some("nord".into());
        settings
            .columns
            .get_mut("did:plc:me")
            .unwrap()
            .push(ColumnSource::Notifications);
        store.save(&settings).unwrap();
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert_eq!(
            saved["columns"],
            serde_json::json!({
                "did:plc:me": [{"kind": "following"}, {"kind": "notifications"}, list],
                "did:plc:work": [list],
            })
        );
    }

    // A folder chosen in the settings is tried with a file of its own. A
    // link someone left at that name is not written through: the file it
    // points to keeps what it holds.
    #[cfg(unix)]
    #[test]
    fn checking_a_folder_never_writes_through_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("notes.txt");
        fs::write(&target, "keep me 📝").unwrap();
        let folder = dir.path().join("chosen");
        fs::create_dir(&folder).unwrap();
        std::os::unix::fs::symlink(
            &target,
            folder.join(format!(".bsky-write-test-{}-0", std::process::id())),
        )
        .unwrap();
        assert!(check_writable(&folder).is_ok());
        assert_eq!(fs::read_to_string(&target).unwrap(), "keep me 📝");
    }

    // The locale is read as every program reads it: LC_ALL, then
    // LC_MESSAGES, then LANG, the first one set; C is no language, so
    // English.
    #[rstest::rstest]
    #[case(&[("LANG", "ja_JP.UTF-8")], Some(crate::i18n::Lang::Ja))]
    #[case(&[("LC_ALL", "C.UTF-8"), ("LANG", "ja_JP.UTF-8")], None)]
    #[case(&[("LC_ALL", "ru_RU.UTF-8"), ("LANG", "ja_JP.UTF-8")], Some(crate::i18n::Lang::Ru))]
    #[case(&[("LC_MESSAGES", "pt_BR"), ("LANG", "en_US.UTF-8")], Some(crate::i18n::Lang::Pt))]
    #[case(&[("LC_ALL", ""), ("LANG", "zh_CN.UTF-8")], Some(crate::i18n::Lang::Zh))]
    #[case(&[("LANG", "sv_SE.UTF-8")], None)]
    #[case(&[], None)]
    fn the_locale_is_the_first_variable_set(
        #[case] vars: &[(&str, &str)],
        #[case] want: Option<crate::i18n::Lang>,
    ) {
        let env = Environment::from_vars(|k| {
            vars.iter()
                .find(|(name, _)| *name == k)
                .map(|(_, v)| (*v).to_string())
        });
        assert_eq!(
            env.locale.as_deref().and_then(crate::i18n::Lang::from_code),
            want
        );
    }

    fn env(var: &str) -> Environment {
        Environment {
            download_dir: Some(format!("{var}/downloads")),
            cache_dir: Some(format!("{var}/cache")),
            video_service: Some(format!("https://{var}.example/")),
            browser: Some(format!("{var}-browser")),
            graphics: None,
            locale: None,
        }
    }

    fn file(val: &str) -> Settings {
        Settings {
            download_dir: Some(format!("{val}/downloads")),
            cache_dir: Some(format!("{val}/cache")),
            video_service: Some(format!("https://{val}.example")),
            browser: Some(format!("{val}-browser")),
            ..Settings::default()
        }
    }

    #[test]
    fn the_environment_wins_over_the_file_which_wins_over_the_default() {
        let none = (Environment::default(), Settings::default());
        let only_file = (Environment::default(), file("写真👨\u{200d}👩\u{200d}👧"));
        let both = (env("var"), file("file"));
        let only_env = (env("var"), Settings::default());

        assert_eq!(
            download_dir(&none.0, &none.1),
            (default_download_dir(), Source::Default)
        );
        assert_eq!(
            download_dir(&only_file.0, &only_file.1),
            (
                Some(PathBuf::from("写真👨\u{200d}👩\u{200d}👧/downloads")),
                Source::File
            )
        );
        assert_eq!(
            download_dir(&both.0, &both.1),
            (Some(PathBuf::from("var/downloads")), Source::Env)
        );
        assert_eq!(
            download_dir(&only_env.0, &only_env.1),
            (Some(PathBuf::from("var/downloads")), Source::Env)
        );

        let platform = Some(PathBuf::from("/c"));
        assert_eq!(
            cache_dir_from(&none.0, &none.1, platform.clone()),
            (Some(PathBuf::from("/c").join("bsky")), Source::Default)
        );
        assert_eq!(
            cache_dir_from(&none.0, &none.1, None),
            (None, Source::Default)
        );
        assert_eq!(
            cache_dir_from(&only_file.0, &only_file.1, platform.clone()),
            (
                Some(PathBuf::from("写真👨\u{200d}👩\u{200d}👧/cache")),
                Source::File
            )
        );
        assert_eq!(
            cache_dir_from(&both.0, &both.1, platform),
            (Some(PathBuf::from("var/cache")), Source::Env)
        );

        assert_eq!(
            video_service(&none.0, &none.1),
            (
                crate::api::DEFAULT_VIDEO_SERVICE.to_string(),
                Source::Default
            )
        );
        assert_eq!(video_service(&only_file.0, &only_file.1).1, Source::File);
        // A trailing slash is dropped, whoever wrote it.
        assert_eq!(
            video_service(&both.0, &both.1),
            ("https://var.example".to_string(), Source::Env)
        );

        assert_eq!(browser(&none.0, &none.1), (None, Source::Default));
        assert_eq!(
            browser(&only_file.0, &only_file.1),
            (
                Some("写真👨\u{200d}👩\u{200d}👧-browser".to_string()),
                Source::File
            )
        );
        assert_eq!(
            browser(&both.0, &both.1),
            (Some("var-browser".to_string()), Source::Env)
        );
    }

    #[test]
    fn off_keeps_no_cache_from_the_environment_or_the_file() {
        let platform = Some(PathBuf::from("/c"));
        let off_env = Environment {
            cache_dir: Some("off".into()),
            ..Environment::default()
        };
        assert_eq!(
            cache_dir_from(&off_env, &file("f"), platform.clone()),
            (None, Source::Env)
        );
        let off_file = Settings {
            cache_dir: Some("off".into()),
            ..Settings::default()
        };
        assert_eq!(
            cache_dir_from(&Environment::default(), &off_file, platform),
            (None, Source::File)
        );
    }

    #[test]
    fn a_blank_value_in_the_file_is_the_default() {
        let blank = Settings {
            download_dir: Some("  ".into()),
            browser: Some(String::new()),
            ..Settings::default()
        };
        let none = Environment::default();
        assert_eq!(download_dir(&none, &blank).1, Source::Default);
        assert_eq!(browser(&none, &blank), (None, Source::Default));
    }

    #[test]
    fn a_folder_that_cannot_be_written_is_refused_with_the_reason() {
        let dir = tempfile::tempdir().unwrap();
        let new = dir.path().join("写真👨\u{200d}👩\u{200d}👧").join("bsky");
        assert_eq!(check_writable(&new), Ok(()));
        assert!(new.is_dir(), "created");
        assert_eq!(fs::read_dir(&new).unwrap().count(), 0, "nothing left");
        // A file where the folder would be.
        let file = dir.path().join("a-file");
        fs::write(&file, "x").unwrap();
        let err = check_writable(&file.join("sub")).unwrap_err();
        assert!(err.starts_with("cannot write to "), "{err}");
    }

    #[test]
    fn an_empty_variable_does_not_fix_a_setting() {
        let env = Environment::from_vars(|k| match k {
            "BSKY_GRAPHICS" => Some("kitty".into()),
            "BSKY_DOWNLOAD_DIR" => Some("  ".into()),
            "BSKY_BROWSER" => Some(String::new()),
            "BSKY_CACHE_DIR" => Some("off".into()),
            _ => None,
        });
        assert_eq!(
            env,
            Environment {
                graphics: Some("kitty".into()),
                cache_dir: Some("off".into()),
                ..Environment::default()
            }
        );
    }

    #[test]
    fn pictures_off_is_kept_beside_the_theme_and_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path());
        fs::write(store.path(), br#"{"theme":"nord","future":1}"#).unwrap();
        let (mut settings, _) = store.load();
        assert!(!settings.pictures_off());
        settings.pictures = Some("off".into());
        store.save(&settings).unwrap();
        let (again, _) = store.load();
        assert!(again.pictures_off());
        let saved: serde_json::Value =
            serde_json::from_slice(&fs::read(store.path()).unwrap()).unwrap();
        assert_eq!(
            saved,
            serde_json::json!({"theme": "nord", "pictures": "off", "future": 1})
        );
    }

    /// A session file of its own in `dir`, as an account's is.
    fn session_store(dir: impl Into<PathBuf>) -> SessionStore {
        SessionStore {
            dir: dir.into(),
            file: "account.json".into(),
        }
    }

    fn sample() -> Session {
        Session {
            service: "https://pds.example".into(),
            did: "did:plc:alice".into(),
            handle: "alice.test".into(),
            access_jwt: "access".into(),
            refresh_jwt: "refresh".into(),
        }
    }

    fn account(did: &str, handle: &str) -> Session {
        Session {
            did: did.into(),
            handle: handle.into(),
            ..sample()
        }
    }

    // New tokens of an account logged out meanwhile (a load of it was still
    // on its way) do not bring its file back; one still logged in has them
    // saved.
    #[test]
    fn new_tokens_are_kept_only_for_an_account_still_logged_in() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("did:plc:a", "a.test")).unwrap();
        store.save(&account("did:plc:b", "b.test")).unwrap();
        let fresh = |did: &str, handle: &str| Session {
            access_jwt: "new-access".into(),
            ..account(did, handle)
        };
        store.remove("did:plc:a").unwrap();
        store
            .store_for("did:plc:a")
            .update(&fresh("did:plc:a", "a.test"))
            .unwrap();
        assert_eq!(store.find("did:plc:a").unwrap(), None);
        store
            .store_for("did:plc:b")
            .update(&fresh("did:plc:b", "b.test"))
            .unwrap();
        assert_eq!(
            store.find("did:plc:b").unwrap().unwrap().access_jwt,
            "new-access"
        );
    }

    // An account file that cannot be read is not a dead end: logging every
    // account out takes it with the others.
    #[test]
    fn logging_every_account_out_takes_a_broken_file_too() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("did:plc:a", "a.test")).unwrap();
        fs::write(
            dir.path().join("accounts").join("did_plc_b.json"),
            "{not json",
        )
        .unwrap();
        assert!(store.list().is_err());
        let gone = store.remove_all().unwrap();
        assert_eq!(
            gone.iter().map(|s| s.handle.as_str()).collect::<Vec<_>>(),
            ["a.test"]
        );
        assert_eq!(store.list().unwrap(), Vec::new());
        assert_eq!(store.current().unwrap(), None);
    }

    #[test]
    fn accounts_are_listed_by_handle_and_found_by_handle_or_did() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        assert_eq!(store.current().unwrap(), None);
        store
            .save(&account("did:plc:work", "Work.example"))
            .unwrap();
        store
            .save(&account("did:web:me.example%3A8080", "alice.test"))
            .unwrap();
        let handles: Vec<_> = store
            .list()
            .unwrap()
            .into_iter()
            .map(|s| s.handle)
            .collect();
        assert_eq!(handles, ["alice.test", "Work.example"]);
        // The last one saved is in use.
        assert_eq!(store.current().unwrap().unwrap().handle, "alice.test");
        assert_eq!(
            store.find("@work.EXAMPLE").unwrap().unwrap().did,
            "did:plc:work"
        );
        assert_eq!(
            store.find("did:plc:work").unwrap().unwrap().handle,
            "Work.example"
        );
        assert_eq!(store.find("nobody.test").unwrap(), None);
        store.set_current("did:plc:work").unwrap();
        assert_eq!(store.current().unwrap().unwrap().did, "did:plc:work");
        // File names hold no colon or percent sign.
        for e in fs::read_dir(dir.path().join(ACCOUNTS_DIR)).unwrap() {
            let name = e.unwrap().file_name().to_string_lossy().into_owned();
            assert!(!name.contains(':') && !name.contains('%'), "{name}");
        }
    }

    #[test]
    fn logging_one_account_out_keeps_the_other_and_moves_on_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        let a = account("did:plc:a", "a.test");
        let b = account("did:plc:b", "b.test");
        store.save(&a).unwrap();
        store.save(&b).unwrap();
        assert!(store.remove("did:plc:b").unwrap());
        assert_eq!(store.current().unwrap(), Some(a.clone()));
        assert_eq!(store.list().unwrap(), vec![a]);
        assert!(!store.remove("did:plc:b").unwrap());
        assert!(store.remove("did:plc:a").unwrap());
        assert_eq!(store.current().unwrap(), None);
    }

    #[test]
    fn a_refresh_rewrites_only_its_own_account() {
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("did:plc:a", "a.test")).unwrap();
        store.save(&account("did:plc:b", "b.test")).unwrap();
        let mut refreshed = account("did:plc:a", "a.test");
        refreshed.refresh_jwt = "rotated".into();
        store.store_for("did:plc:a").save(&refreshed).unwrap();
        let all = store.list().unwrap();
        assert_eq!(all[0].refresh_jwt, "rotated");
        assert_eq!(all[1].refresh_jwt, "refresh");
        // The one in use stays the one in use.
        assert_eq!(store.current().unwrap().unwrap().did, "did:plc:b");
    }

    #[cfg(unix)]
    #[test]
    fn account_files_are_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = AccountStore::open(dir.path());
        store.save(&account("did:plc:a", "a.test")).unwrap();
        let path = store.store_for("did:plc:a").path();
        let mode = fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn load_without_file_is_none() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(session_store(dir.path()).load().unwrap(), None);
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let store = session_store(dir.path().join("nested"));
        store.save(&sample()).unwrap();
        assert_eq!(store.load().unwrap(), Some(sample()));
    }

    #[test]
    fn clear_reports_whether_a_session_existed() {
        let dir = tempfile::tempdir().unwrap();
        let store = session_store(dir.path());
        assert!(!store.clear().unwrap());
        store.save(&sample()).unwrap();
        assert!(store.clear().unwrap());
        assert_eq!(store.load().unwrap(), None);
    }

    // was: the temporary file, which holds the tokens, was left behind when
    // the rename failed.
    #[test]
    fn a_failed_save_leaves_no_temporary_file_with_the_tokens() {
        let dir = tempfile::tempdir().unwrap();
        let store = session_store(dir.path());
        // A directory where the session file goes: the rename cannot happen.
        fs::create_dir(store.path()).unwrap();
        assert!(store.save(&sample()).is_err());
        let left: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect();
        assert!(left.is_empty(), "left behind: {left:?}");
    }

    #[test]
    fn corrupt_file_is_an_io_error_with_a_hint() {
        let dir = tempfile::tempdir().unwrap();
        let store = session_store(dir.path());
        fs::write(store.path(), b"{not json").unwrap();
        let err = store.load().unwrap_err();
        assert_eq!(err.kind(), crate::error::Kind::Io);
        assert!(
            err.to_string().contains("\nhint: run `bsky logout --all`"),
            "{err}"
        );
    }

    // A file an editor saved with a byte order mark is read as it is meant.
    #[test]
    fn settings_saved_with_a_byte_order_mark_are_read() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(
            dir.path().join(SETTINGS_FILE),
            "\u{feff}{\"theme\": \"nord\"}\n",
        )
        .unwrap();
        let (settings, warning) = SettingsStore::new(dir.path()).load();
        assert_eq!(warning, None);
        assert_eq!(settings.theme.as_deref(), Some("nord"));
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
        let store = session_store(dir.path());
        store.save(&sample()).unwrap();
        store.save(&sample()).unwrap();
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["account.json"]);
    }

    #[cfg(unix)]
    #[test]
    fn session_file_is_owner_only() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let store = session_store(dir.path());
        fs::write(store.path(), b"old").unwrap();
        fs::set_permissions(store.path(), fs::Permissions::from_mode(0o644)).unwrap();
        store.save(&sample()).unwrap();
        let mode = fs::metadata(store.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
