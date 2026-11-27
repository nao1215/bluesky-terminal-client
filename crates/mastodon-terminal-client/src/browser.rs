//! Opening the login page in the web browser, with what the system already
//! has: `xdg-open` on Linux and the BSDs, `open` on macOS, the URL handler
//! on Windows. `MSTDN_BROWSER` names another program, as `MSKY_BROWSER`
//! does for msky and `BSKY_BROWSER` for bsky.

use std::process::{Command, Stdio};

/// Environment variable naming the program that opens links.
pub const BROWSER_ENV: &str = "MSTDN_BROWSER";

/// The program `MSTDN_BROWSER` names, if it names one.
pub fn named() -> Option<String> {
    std::env::var(BROWSER_ENV)
        .ok()
        .filter(|v| !v.trim().is_empty())
}

/// The program and arguments that open `url`: `program` when one is named,
/// else the system's opener.
fn opener(url: &str, program: Option<&str>) -> (String, Vec<String>) {
    if let Some(program) = program {
        return (program.to_string(), vec![url.to_string()]);
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

/// Try to open `url` in the browser; whether the opener started. Only web
/// links are opened. The address is printed either way, so an opener that
/// is not there is no error.
pub fn open(url: &str) -> bool {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return false;
    }
    let (program, args) = opener(url, named().as_deref());
    let mut cmd = Command::new(program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match cmd.spawn() {
        Ok(mut child) => {
            // Reaped in the background; the opener usually hands off and exits.
            std::thread::spawn(move || child.wait());
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_program_gets_the_url_as_its_only_argument() {
        let (p, a) = opener(
            "https://mastodon.social/oauth/authorize?a=1&b=2",
            Some("firefox"),
        );
        assert_eq!(p, "firefox");
        assert_eq!(a, ["https://mastodon.social/oauth/authorize?a=1&b=2"]);
        let (_, a) = opener("https://mastodon.social/", None);
        assert_eq!(a.last().unwrap(), "https://mastodon.social/");
    }

    #[test]
    fn what_is_not_a_web_link_is_not_opened() {
        assert!(!open("file:///etc/passwd"));
        assert!(!open("javascript:alert(1)"));
    }
}
