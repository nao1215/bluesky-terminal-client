//! User-owned configuration and signing keys; reading needs only a public key.

use crate::{
    api::{event::bytes, nip19::Identifier, nip46::Bunker},
    error::{Error, Result},
};
use k256::schnorr::{SigningKey, VerifyingKey};
use serde::Deserialize;
use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
    time::Duration,
};
use zeroize::Zeroizing;

/// Environment variable that overrides the config directory.
pub(crate) const CONFIG_DIR_ENV: &str = "NSTR_CONFIG_DIR";

pub(crate) fn config_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os(CONFIG_DIR_ENV).filter(|v| !v.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    dirs::config_dir().map(|d| d.join("nstr")).ok_or_else(|| {
        Error::file(format!(
            "cannot locate the config directory; set {CONFIG_DIR_ENV}"
        ))
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Settings {
    public_key: String,
    key_file: Option<PathBuf>,
    bunker: Option<String>,
    relays: Vec<String>,
    #[serde(default = "default_timeout")]
    timeout_seconds: u64,
}

fn default_timeout() -> u64 {
    10
}

pub(crate) struct Config {
    pub(crate) public: String,
    pub(crate) relays: Vec<String>,
    pub(crate) timeout: Duration,
    key_file: Option<PathBuf>,
    pub(crate) bunker: Option<Bunker>,
}

impl Config {
    pub(crate) fn load() -> Result<Self> {
        let path = config_dir()?.join("config.json");
        let file = File::open(&path)
            .map_err(|e| Error::file(format!("cannot open config {}: {e}", path.display())))?;
        let settings: Settings = serde_json::from_reader(file.take(65537))
            .map_err(|e| Error::file(format!("cannot parse config: {e}")))?;
        let key = if settings.public_key.len() == 64 {
            bytes::<32>(&settings.public_key)
        } else {
            match Identifier::decode(&settings.public_key) {
                Ok(Identifier::Public(k) | Identifier::Profile { public: k, .. }) => Ok(k),
                _ => Err(Error::file("publicKey must be hex, npub or nprofile")),
            }
        }
        .map_err(|_invalid| Error::file("publicKey must be hex, npub or nprofile"))?;
        VerifyingKey::from_slice(&key)
            .map_err(|_invalid| Error::file("publicKey is not a secp256k1 public key"))?;
        if settings.key_file.is_some() && settings.bunker.is_some() {
            return Err(Error::file(
                "keyFile and bunker cannot be configured together",
            ));
        }
        if !(1..=300).contains(&settings.timeout_seconds) {
            return Err(Error::file("timeoutSeconds must be between 1 and 300"));
        }
        let relays = relay_urls(settings.relays)?;
        let bunker = settings.bunker.map(|uri| Bunker::parse(&uri)).transpose()?;
        let key_file = settings.key_file.map(|p| {
            if p.is_absolute() {
                p
            } else {
                path.parent().unwrap_or(Path::new(".")).join(p)
            }
        });
        Ok(Self {
            public: hex::encode(key),
            relays,
            timeout: Duration::from_secs(settings.timeout_seconds),
            key_file,
            bunker,
        })
    }

    pub(crate) fn signing_key(&self) -> Result<SigningKey> {
        let path = self
            .key_file
            .as_ref()
            .ok_or_else(|| Error::usage("posting requires keyFile or bunker in the config"))?;
        let meta = std::fs::symlink_metadata(path)
            .map_err(|e| Error::file(format!("cannot inspect key file: {e}")))?;
        if !meta.file_type().is_file() {
            return Err(Error::file("keyFile must be a regular file, not a symlink"));
        }
        let file =
            File::open(path).map_err(|e| Error::file(format!("cannot open key file: {e}")))?;
        let meta = file.metadata().map_err(Error::file)?;
        if !meta.is_file() || meta.len() > 1024 {
            return Err(Error::file(
                "key file must be a regular file of at most 1024 bytes",
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if meta.permissions().mode() & 0o077 != 0 {
                return Err(Error::file(
                    "key file must be accessible only to its owner; run chmod 600 on it",
                ));
            }
        }
        let mut content = Zeroizing::new(String::new());
        file.take(1025)
            .read_to_string(&mut content)
            .map_err(|_invalid| Error::file("cannot read key file as UTF-8"))?;
        if content.len() > 1024 {
            return Err(Error::file("key file exceeds 1024 bytes"));
        }
        let raw = Zeroizing::new(if content.trim().len() == 64 {
            bytes::<32>(content.trim()).map_err(|_invalid| {
                Error::file("key file must contain one hex or nsec secret key")
            })?
        } else {
            match Identifier::decode(content.trim()) {
                Ok(Identifier::Secret(k)) => k,
                _ => {
                    return Err(Error::file(
                        "key file must contain one hex or nsec secret key",
                    ));
                }
            }
        });
        let key = SigningKey::from_slice(raw.as_ref())
            .map_err(|_invalid| Error::file("key file contains an invalid secret scalar"))?;
        if hex::encode(key.verifying_key().to_bytes()) != self.public {
            return Err(Error::file("key file does not match publicKey"));
        }
        Ok(key)
    }
}

/// Normalize user-configured relay addresses without reflecting credentials in errors.
pub(crate) fn relay_urls(values: Vec<String>) -> Result<Vec<String>> {
    if values.is_empty() || values.len() > 16 {
        return Err(Error::file("configure between 1 and 16 relays"));
    }
    let mut relays = Vec::new();
    for value in values {
        let url = url::Url::parse(&value)
            .map_err(|_invalid| Error::file("each relay must be a ws:// or wss:// URL"))?;
        if !matches!(url.scheme(), "ws" | "wss")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.fragment().is_some()
            || value.chars().any(char::is_whitespace)
        {
            return Err(Error::file(
                "relay URLs require ws:// or wss://, a host, no credentials and no fragment",
            ));
        }
        let normalized = url.to_string();
        if !relays.contains(&normalized) {
            relays.push(normalized);
        }
    }
    Ok(relays)
}
