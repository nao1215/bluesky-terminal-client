//! The languages mstdn speaks to the person at the keyboard, by bsky's and
//! msky's mechanism: English and Japanese. Every such string is written in
//! English in the code and looked up here by that English text: a language
//! that lacks one shows the English, so a new string never shows as a blank
//! or a key.
//!
//! The commands' output, errors and `--json` stay in English, as msky's do,
//! so scripts read the same words everywhere; what is translated is what
//! only a person reads: the full-screen client, the login's directions on
//! stderr and the page the browser is sent back to. The language is
//! `LC_ALL`, `LC_MESSAGES` or `LANG`, the first set, else English.
//!
//! Strings are marked where they are written with [`t`], [`tf`] or `n!`.
//! A test reads the marks from the source and checks that every language
//! has every string.
//!
//! A thread may be set on its own, which is what each test does: in a test
//! build a thread's choice stays its own, so tests running side by side do
//! not change each other's, and a thread that chose nothing is in English.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

mod ja;

use terminal_kit::i18n::Catalog;
pub(crate) use terminal_kit::{i18n::Lang, n};

pub(crate) const ALL: [Lang; 2] = [Lang::En, Lang::Ja];
static CATALOG: Catalog = Catalog::new(&[(Lang::Ja, ja::TABLE)]);
pub(crate) fn from_code(code: &str) -> Option<Lang> {
    Lang::from_code(code).filter(|l| ALL.contains(l))
}
pub(crate) fn from_locale(get: impl Fn(&str) -> Option<String>) -> Lang {
    from_code(terminal_kit::locale::language_from_locale(get)).unwrap_or(Lang::En)
}

/// The process's language, by its place in [`ALL`].
static PROCESS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static CURRENT: Cell<Option<Lang>> = const { Cell::new(None) };
}

/// Show everything from now on in `lang`: on this thread, and, outside
/// tests, on every thread.
pub(crate) fn set(lang: Lang) {
    CURRENT.with(|c| c.set(Some(lang)));
    #[cfg(not(test))]
    PROCESS.store(
        ALL.iter().position(|l| *l == lang).unwrap_or(0),
        Ordering::Relaxed,
    );
}

/// The language in use on this thread: its own choice, else the process's.
pub(crate) fn current() -> Lang {
    CURRENT
        .with(Cell::get)
        .unwrap_or_else(|| ALL[PROCESS.load(Ordering::Relaxed).min(ALL.len() - 1)])
}

pub(crate) fn t(s: &str) -> &str {
    CATALOG.t(current(), s)
}
pub(crate) fn tf(template: &str, args: &[&str]) -> String {
    CATALOG.tf(current(), template, args)
}

#[cfg(test)]
mod tests;
