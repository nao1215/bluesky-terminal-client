//! Browser selection for this client; process spawning is shared.
pub(crate) const BROWSER_ENV: &str = "MSTDN_BROWSER";
pub(crate) fn open(url: &str) -> bool {
    open_with(url, None)
}
pub(crate) fn open_with(url: &str, program: Option<&str>) -> bool {
    terminal_kit::browser::open(url, BROWSER_ENV, program)
}
pub(crate) fn named() -> Option<String> {
    std::env::var(BROWSER_ENV)
        .ok()
        .filter(|s| !s.trim().is_empty())
}
