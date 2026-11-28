//! Open web URLs using an environment override, saved program, or the system browser.
use std::process::{Command, Stdio};

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
pub fn open(url: &str, env: &str, program: Option<&str>) -> bool {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return false;
    }
    let named = std::env::var(env)
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| program.map(str::to_string).filter(|v| !v.trim().is_empty()));
    let (program, args) = opener(url, named.as_deref());
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
        let (p, a) = opener("https://misskey.io/miauth/x?a=1&b=2", Some("firefox"));
        assert_eq!(p, "firefox");
        assert_eq!(a, ["https://misskey.io/miauth/x?a=1&b=2"]);
        let (_, a) = opener("https://misskey.io/", None);
        assert_eq!(a.last().unwrap(), "https://misskey.io/");
    }

    #[test]
    fn what_is_not_a_web_link_is_not_opened() {
        assert!(!open("file:///etc/passwd", "STC_TEST_BROWSER", None));
        assert!(!open("javascript:alert(1)", "STC_TEST_BROWSER", None));
    }
}
