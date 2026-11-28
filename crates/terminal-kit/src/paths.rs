//! Application directories and private atomic file writes.
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

/// Prefer a nonempty override verbatim, otherwise append the app name to the platform directory.
/// Paths need not be UTF-8; `off` is an ordinary config directory name.
pub fn resolve_app_dir(
    override_dir: Option<&OsStr>,
    platform: Option<&Path>,
    app: &str,
) -> Option<PathBuf> {
    override_dir
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| platform.map(|p| p.join(app)))
}

/// Resolve a cache directory, with the exact override `off` disabling the cache.
pub fn resolve_cache_dir(
    override_dir: Option<&OsStr>,
    platform: Option<&Path>,
    app: &str,
) -> Option<PathBuf> {
    if override_dir == Some(OsStr::new("off")) {
        None
    } else {
        resolve_app_dir(override_dir, platform, app)
    }
}

/// Write through a synced temporary file, then rename it over the destination.
/// Unix files are restricted to 0600, including a leftover temporary file.
/// Failed writes remove the temporary file or report why it remains.
/// The PID-based temporary name does not serialize concurrent writes within one process.
pub fn write_private(path: &Path, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    // The process id keeps two running copies of a client from writing the same
    // temporary file, which on Windows would make one of the renames fail.
    let tmp = path.with_extension(format!("json.{}.tmp", std::process::id()));
    let write = || -> std::io::Result<()> {
        let mut file = open_private(&tmp)?;
        file.write_all(data)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    };
    // The temporary file holds the tokens; it must not be left behind, and
    // when it cannot be removed the error says where it is.
    write().map_err(|e| match fs::remove_file(&tmp) {
        Err(left) if left.kind() != std::io::ErrorKind::NotFound => std::io::Error::new(
            e.kind(),
            format!("{e}; {} is left behind: {left}", tmp.display()),
        ),
        _ => e,
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

    #[test]
    fn directory_resolution_keeps_empty_off_and_missing_distinct() {
        let platform = Some(Path::new("platform"));
        assert_eq!(
            resolve_app_dir(Some(OsStr::new("")), platform, "app"),
            Some(PathBuf::from("platform/app"))
        );
        assert_eq!(
            resolve_app_dir(Some(OsStr::new("off")), platform, "app"),
            Some(PathBuf::from("off"))
        );
        assert_eq!(
            resolve_cache_dir(Some(OsStr::new("off")), platform, "app"),
            None
        );
        assert_eq!(
            resolve_cache_dir(Some(OsStr::new("OFF")), None, "app"),
            Some(PathBuf::from("OFF"))
        );
        assert_eq!(resolve_app_dir(None, None, "app"), None);
    }

    #[cfg(unix)]
    #[test]
    fn paths_need_not_be_utf8_and_leftover_permissions_are_tightened() {
        use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
        let path = OsStr::from_bytes(b"cache-\xff");
        assert_eq!(
            resolve_cache_dir(Some(path), None, "app"),
            Some(PathBuf::from(path))
        );
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("settings.json");
        let tmp = dest.with_extension(format!("json.{}.tmp", std::process::id()));
        fs::write(&tmp, b"old").unwrap();
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o666)).unwrap();
        write_private(&dest, b"new").unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"new");
        assert_eq!(
            fs::metadata(&dest).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(!tmp.exists());
    }

    #[test]
    fn a_failed_rename_removes_the_private_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("occupied.json");
        fs::create_dir(&dest).unwrap();
        assert!(write_private(&dest, b"secret").is_err());
        assert!(
            !dest
                .with_extension(format!("json.{}.tmp", std::process::id()))
                .exists()
        );
    }
}
