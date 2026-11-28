//! The languages the client is shown in. Every string the user reads is
//! written in English in the code and looked up here by that English text:
//! a language that lacks one shows the English, so a new string never shows
//! as a blank or a key.
//!
//! Strings are marked where they are written, with [`n!`] when they are
//! data looked up later (a hint, a help line, a title) and with [`t`] or
//! [`tf`] when they are shown at once. A test reads the marks from the
//! source and checks that every language has every string.
//!
//! The language chosen is the whole process's: the worker and player
//! threads write their messages in it too. A thread may be set on its own,
//! which is what each test does: in a test build a thread's choice stays
//! its own, so tests running side by side do not change each other's, and
//! a thread that chose nothing is in English.

use std::cell::Cell;
use std::sync::atomic::{AtomicUsize, Ordering};

mod de;
mod es;
mod fr;
mod ja;
mod ko;
mod pt;
mod ru;
mod zh;

use terminal_kit::i18n::Catalog;
pub(crate) use terminal_kit::{i18n::Lang, n};

pub(crate) const ALL: [Lang; 9] = [
    Lang::En,
    Lang::Ja,
    Lang::Zh,
    Lang::Ko,
    Lang::Ru,
    Lang::Es,
    Lang::Fr,
    Lang::De,
    Lang::Pt,
];
static CATALOG: Catalog = Catalog::new(&[
    (Lang::Ja, ja::TABLE),
    (Lang::Zh, zh::TABLE),
    (Lang::Ko, ko::TABLE),
    (Lang::Ru, ru::TABLE),
    (Lang::Es, es::TABLE),
    (Lang::Fr, fr::TABLE),
    (Lang::De, de::TABLE),
    (Lang::Pt, pt::TABLE),
]);
pub(crate) fn from_code(code: &str) -> Option<Lang> {
    Lang::from_code(code).filter(|l| ALL.contains(l))
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
