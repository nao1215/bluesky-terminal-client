//! Named configuration files; secrets stay in the referenced key file or bunker.
use crate::{
    config::config_dir,
    error::{Error, Result},
};
use std::path::PathBuf;

pub(crate) fn name(value: &str) -> Result<&str> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::usage(
            "account names use 1–64 ASCII letters, digits, - or _",
        ));
    }
    Ok(value)
}
pub(crate) fn path(account: Option<&str>) -> Result<PathBuf> {
    let dir = config_dir()?;
    let selected = if account.is_none() {
        match std::fs::read_to_string(dir.join("active-account")) {
            Ok(s) => Some(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(Error::file(e)),
        }
    } else {
        None
    };
    match account.or(selected.as_deref()) {
        Some("default") => Ok(dir.join("config.json")),
        Some(a) => Ok(dir.join("accounts").join(format!("{}.json", name(a)?))),
        None => Ok(dir.join("config.json")),
    }
}
pub(crate) fn list() -> Result<Vec<String>> {
    let root = config_dir()?;
    let mut names = if root.join("config.json").exists() {
        vec!["default".into()]
    } else {
        Vec::new()
    };
    let dir = root.join("accounts");
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(names),
        Err(e) => return Err(Error::file(e)),
    };
    for entry in entries {
        let p = entry.map_err(Error::file)?.path();
        if p.extension().is_some_and(|s| s == "json")
            && let Some(s) = p.file_stem().and_then(|s| s.to_str())
        {
            names.push(name(s)?.to_owned());
        }
    }
    names.sort();
    Ok(names)
}
pub(crate) fn select(account: &str) -> Result<()> {
    crate::config::Config::load_account(Some(account))?;
    std::fs::write(config_dir()?.join("active-account"), name(account)?).map_err(Error::file)
}

/// Save only a new configuration, validating it locally before keeping it.
pub(crate) fn add(account: Option<&str>, value: serde_json::Value) -> Result<()> {
    use std::io::Write;
    if !value.is_object() {
        return Err(Error::usage("account configuration must be a JSON object"));
    }
    let path = path(account)?;
    std::fs::create_dir_all(
        path.parent()
            .ok_or_else(|| Error::file("account directory missing"))?,
    )
    .map_err(Error::file)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&path).map_err(Error::file)?;
    let result = file
        .write_all(value.to_string().as_bytes())
        .map_err(Error::file)
        .and_then(|()| crate::config::Config::load_account(account).map(|_| ()));
    if result.is_err() {
        drop(file);
        std::fs::remove_file(path).map_err(Error::file)?;
    }
    result
}

pub(crate) fn remove(account: &str) -> Result<()> {
    std::fs::remove_file(path(Some(account))?).map_err(Error::file)?;
    let active = config_dir()?.join("active-account");
    match std::fs::read_to_string(&active) {
        Ok(s) if s == account => std::fs::remove_file(active).map_err(Error::file),
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(Error::file(e)),
    }
}
