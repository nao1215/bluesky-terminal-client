//! Opening a post's link in the user's web browser, with what the system
//! already has: `xdg-open` on Linux and the BSDs, `open` on macOS, the URL
//! handler on Windows. `BSKY_BROWSER` names another program to open links with.

use std::process::{Command, Stdio};

use crate::error::{Error, Kind, Result};

/// Environment variable naming the program that opens links.
pub const BROWSER_ENV: &str = "BSKY_BROWSER";

/// The program and arguments that open `url` here.
fn opener(url: &str) -> (String, Vec<String>) {
    if let Some(program) = std::env::var_os(BROWSER_ENV).filter(|v| !v.is_empty()) {
        return (
            program.to_string_lossy().into_owned(),
            vec![url.to_string()],
        );
    }
    if cfg!(target_os = "macos") {
        ("open".into(), vec![url.into()])
    } else if cfg!(windows) {
        // Not `cmd /c start`: cmd would read `&` in a URL as a command.
        (
            "rundll32".into(),
            vec!["url.dll,FileProtocolHandler".into(), url.into()],
        )
    } else {
        ("xdg-open".into(), vec![url.into()])
    }
}

/// What opens links when `BSKY_BROWSER` is not set, as the settings
/// screen names it.
pub fn system_opener() -> &'static str {
    if cfg!(target_os = "macos") {
        "open"
    } else if cfg!(windows) {
        "the Windows URL handler"
    } else {
        "xdg-open"
    }
}

/// Open `url` in the browser. Only web links are opened, so a post cannot
/// make bsky run a file or a command.
pub fn open(url: &str) -> Result<()> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(Error::new(Kind::Usage, format!("not a web link: {url}")));
    }
    let (program, args) = opener(url);
    spawn(&program, &args)
}

fn spawn(program: &str, args: &[String]) -> Result<()> {
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // A browser that runs in the terminal must not take over the screen bsky
    // is drawing on.
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    match cmd.spawn() {
        Ok(mut child) => {
            // Reaped in the background; the opener usually hands off and exits.
            std::thread::spawn(move || child.wait());
            Ok(())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Err(Error::io(format!(
            "cannot open the link: {program} was not found"
        ))
        .with_hint(format!("set {BROWSER_ENV} to the program that opens links"))),
        Err(e) => Err(Error::io(format!(
            "cannot open the link with {program}: {e}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_links_are_opened() {
        for url in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "/usr/bin/true",
            "",
        ] {
            let e = open(url).unwrap_err();
            assert!(e.message().starts_with("not a web link"), "{url}: {e}");
        }
    }

    #[test]
    fn a_missing_opener_says_so() {
        let e = spawn("/definitely/not/a/browser", &["https://a.test".into()]).unwrap_err();
        assert!(e.message().contains("was not found"), "{e}");
        assert!(e.to_string().contains(BROWSER_ENV));
    }

    /// An opener that exists but cannot be run says what went wrong, not
    /// that it is missing.
    #[cfg(unix)]
    #[test]
    fn an_opener_that_cannot_run_says_why() {
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join("browser");
        std::fs::write(&program, "not a program").unwrap();
        let program = program.to_string_lossy().into_owned();
        let e = spawn(&program, &["https://a.test".into()]).unwrap_err();
        assert!(
            e.message()
                .starts_with(&format!("cannot open the link with {program}: ")),
            "{e}"
        );
        assert!(!e.message().contains("was not found"), "{e}");
    }

    #[cfg(unix)]
    #[test]
    fn a_working_opener_is_started_with_the_url() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("opened");
        let program = dir.path().join("browser");
        std::fs::write(
            &program,
            format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", out.display()),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&program).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
        std::fs::set_permissions(&program, perms).unwrap();
        let url = "https://a.test/x?y=1&z=2";
        // Another test forking at the moment the script was written holds
        // its write handle until that child execs, and running the script
        // then fails with "Text file busy" (ETXTBSY). That is the test
        // racing itself, not the opener, so it waits it out.
        let mut started = spawn(&program.to_string_lossy(), &[url.into()]);
        for _ in 0..100 {
            match &started {
                Err(e) if e.message().contains("Text file busy") => {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    started = spawn(&program.to_string_lossy(), &[url.into()]);
                }
                _ => break,
            }
        }
        started.unwrap();
        let begin = std::time::Instant::now();
        while !out.exists() && begin.elapsed() < std::time::Duration::from_secs(5) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
        assert_eq!(std::fs::read_to_string(&out).unwrap(), url);
    }

    #[test]
    fn the_system_opener_takes_the_url_as_one_argument() {
        let url = "https://a.test/x?y=1&z=2";
        let (_, args) = opener(url);
        assert_eq!(args.last().map(String::as_str), Some(url));
    }
}
