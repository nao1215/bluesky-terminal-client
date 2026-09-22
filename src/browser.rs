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

    #[test]
    fn the_system_opener_takes_the_url_as_one_argument() {
        let url = "https://a.test/x?y=1&z=2";
        let (_, args) = opener(url);
        assert_eq!(args.last().map(String::as_str), Some(url));
    }
}
