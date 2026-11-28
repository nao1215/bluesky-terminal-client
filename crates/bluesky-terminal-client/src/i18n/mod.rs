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
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

mod de;
mod es;
mod fr;
mod ja;
mod ko;
mod pt;
mod ru;
mod zh;

/// Mark an English string to be translated where it is shown. It is the
/// string itself, so it can stand in a `const`.
macro_rules! n {
    ($s:literal) => {
        $s
    };
}
pub(crate) use n;

/// A language the client can be shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    En,
    Ja,
    Zh,
    Ko,
    Ru,
    Es,
    Fr,
    De,
    Pt,
}

impl Lang {
    /// Every language, in the order the settings list them.
    pub const ALL: [Lang; 9] = [
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

    /// The code `settings.json` keeps: `en`, `ja`, `zh`...
    pub fn code(self) -> &'static str {
        match self {
            Lang::En => "en",
            Lang::Ja => "ja",
            Lang::Zh => "zh",
            Lang::Ko => "ko",
            Lang::Ru => "ru",
            Lang::Es => "es",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Pt => "pt",
        }
    }

    /// The language's name in itself, as the settings list shows it.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ja => "日本語",
            Lang::Zh => "简体中文",
            Lang::Ko => "한국어",
            Lang::Ru => "Русский",
            Lang::Es => "Español",
            Lang::Fr => "Français",
            Lang::De => "Deutsch",
            Lang::Pt => "Português",
        }
    }

    /// The language a code names: `ja`, and a locale such as `ja_JP.UTF-8`
    /// or `pt-BR`, whatever its case.
    pub fn from_code(code: &str) -> Option<Lang> {
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
            Lang::Zh => zh::TABLE,
            Lang::Ko => ko::TABLE,
            Lang::Ru => ru::TABLE,
            Lang::Es => es::TABLE,
            Lang::Fr => fr::TABLE,
            Lang::De => de::TABLE,
            Lang::Pt => pt::TABLE,
        }
    }

    fn lookup(self) -> &'static HashMap<&'static str, &'static str> {
        static MAPS: [OnceLock<HashMap<&str, &str>>; 9] = [const { OnceLock::new() }; 9];
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
pub fn set(lang: Lang) {
    CURRENT.with(|c| c.set(Some(lang)));
    #[cfg(not(test))]
    PROCESS.store(lang.index(), Ordering::Relaxed);
}

/// The language in use on this thread: its own choice, else the process's.
pub fn current() -> Lang {
    CURRENT
        .with(Cell::get)
        .unwrap_or_else(|| Lang::ALL[PROCESS.load(Ordering::Relaxed).min(Lang::ALL.len() - 1)])
}

/// `s` in the language in use, or `s` itself when it has no translation.
pub fn t(s: &str) -> &str {
    match current() {
        Lang::En => s,
        lang => lang.lookup().get(s).copied().unwrap_or(s),
    }
}

/// [`t`] of `template`, with `{}` in it filled by `args` in order, or `{0}`
/// `{1}`... by position, which a translation may reorder.
pub fn tf(template: &str, args: &[&str]) -> String {
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
