//! The languages mstdn speaks to the person at the keyboard, by bsky's and
//! msky's mechanism: English and Japanese. Every such string is written in
//! English in the code and looked up here by that English text: a language
//! that lacks one shows the English, so a new string never shows as a blank
//! or a key.
//!
//! The commands' output, errors and `--json` stay in English, as msky's do,
//! so scripts read the same words everywhere; what is translated is what
//! only a person reads: the login's directions on stderr and the page the
//! browser is sent back to. The language is `LC_ALL`, `LC_MESSAGES` or
//! `LANG`, the first set, else English.
//!
//! Strings are marked where they are written with [`t`] or [`tf`]. A test
//! reads the marks from the source and checks that every language has
//! every string.
//!
//! A thread may be set on its own, which is what each test does: in a test
//! build a thread's choice stays its own, so tests running side by side do
//! not change each other's, and a thread that chose nothing is in English.

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

mod ja;

/// A language mstdn speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Lang {
    En,
    Ja,
}

impl Lang {
    /// Every language mstdn speaks.
    pub(crate) const ALL: [Lang; 2] = [Lang::En, Lang::Ja];

    /// Its code: `en`, `ja`.
    pub(crate) fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ja => "ja",
        }
    }

    /// The language the locale asks for: the first of `LC_ALL`,
    /// `LC_MESSAGES` and `LANG` that is set (as `get` reads them), else
    /// English.
    pub(crate) fn from_locale(get: impl Fn(&str) -> Option<String>) -> Lang {
        ["LC_ALL", "LC_MESSAGES", "LANG"]
            .into_iter()
            .find_map(|k| get(k).filter(|v| !v.is_empty()))
            .and_then(|v| Lang::from_code(&v))
            .unwrap_or(Lang::En)
    }

    /// The language a code names: `ja`, and a locale such as `ja_JP.UTF-8`
    /// or `pt-BR`, whatever its case.
    pub(crate) fn from_code(code: &str) -> Option<Lang> {
        let head = code
            .split(['_', '-', '.', '@'])
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        Lang::ALL.into_iter().find(|l| l.code() == head)
    }

    /// Where the language is in [`Lang::ALL`].
    fn index(self) -> usize {
        Lang::ALL.iter().position(|l| *l == self).unwrap_or(0)
    }

    /// The translations, by the English they translate.
    fn table(self) -> &'static [(&'static str, &'static str)] {
        match self {
            Lang::En => &[],
            Lang::Ja => ja::TABLE,
        }
    }

    fn lookup(self) -> &'static HashMap<&'static str, &'static str> {
        static MAPS: [OnceLock<HashMap<&str, &str>>; 2] = [const { OnceLock::new() }; 2];
        MAPS[self.index()].get_or_init(|| self.table().iter().copied().collect())
    }
}

/// The process's language, by its place in [`Lang::ALL`].
static PROCESS: AtomicUsize = AtomicUsize::new(0);

thread_local! {
    static CURRENT: Cell<Option<Lang>> = const { Cell::new(None) };
}

/// Show everything from now on in `lang`: on this thread, and, outside
/// tests, on every thread.
pub(crate) fn set(lang: Lang) {
    CURRENT.with(|c| c.set(Some(lang)));
    #[cfg(not(test))]
    PROCESS.store(lang.index(), Ordering::Relaxed);
}

/// The language in use on this thread: its own choice, else the process's.
pub(crate) fn current() -> Lang {
    CURRENT
        .with(Cell::get)
        .unwrap_or_else(|| Lang::ALL[PROCESS.load(Ordering::Relaxed).min(Lang::ALL.len() - 1)])
}

/// `s` in the language in use, or `s` itself when it has no translation.
pub(crate) fn t(s: &str) -> &str {
    match current() {
        Lang::En => s,
        lang => lang.lookup().get(s).copied().unwrap_or(s),
    }
}

/// [`t`] of `template`, with `{}` in it filled by `args` in order, or `{0}`
/// `{1}`... by position, which a translation may reorder.
pub(crate) fn tf(template: &str, args: &[&str]) -> String {
    fill(t(template), args)
}

fn fill(template: &str, args: &[&str]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut next = 0;
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let inside = &after[..close];
        let at = if inside.is_empty() {
            next += 1;
            Some(next - 1)
        } else {
            inside.parse::<usize>().ok()
        };
        match at.and_then(|i| args.get(i)) {
            Some(arg) => out.push_str(arg),
            None => out.push_str(&rest[open..open + close + 2]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests;
