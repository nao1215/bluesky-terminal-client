//! Browser selection for this client; process spawning is shared.
pub(crate) const BROWSER_ENV: &str = "MSKY_BROWSER";
pub(crate) fn open(url: &str) -> bool {
    open_with(url, None)
}
pub(crate) fn open_with(url: &str, program: Option<&str>) -> bool {
    terminal_kit::browser::open(url, BROWSER_ENV, program)
}
