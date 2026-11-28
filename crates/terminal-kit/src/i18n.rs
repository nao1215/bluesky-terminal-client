//! Language metadata, catalogs and placeholder substitution.

mod shared;

use std::collections::HashMap;
use std::sync::OnceLock;

/// Mark an English string to be translated where it is shown. It is the
/// string itself, so it can stand in a `const`.
#[macro_export]
macro_rules! n {
    ($s:literal) => {
        $s
    };
}
pub use crate::n;

/// A language the client can be shown in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// The `en` language.
    En,
    /// The `ja` language.
    Ja,
    /// The `zh` language.
    Zh,
    /// The `ko` language.
    Ko,
    /// The `ru` language.
    Ru,
    /// The `es` language.
    Es,
    /// The `fr` language.
    Fr,
    /// The `de` language.
    De,
    /// The `pt` language.
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
}

/// A translation table indexed by its English source text.
pub type Table = &'static [(&'static str, &'static str)];

/// A client's translations. Missing entries retain their English source text.
pub struct Catalog {
    tables: &'static [(Lang, Table)],
    maps: [OnceLock<HashMap<&'static str, &'static str>>; 9],
}

impl Catalog {
    /// Build a catalog from the client's supported translations.
    pub const fn new(tables: &'static [(Lang, Table)]) -> Self {
        Self {
            tables,
            maps: [const { OnceLock::new() }; 9],
        }
    }

    /// The client's own table for a language, without fallback entries.
    pub fn table(&self, lang: Lang) -> Table {
        self.tables
            .iter()
            .find(|(l, _)| *l == lang)
            .map_or(&[], |(_, t)| *t)
    }

    /// Shared translations used when the client has no entry.
    pub fn fallback_table(&self, lang: Lang) -> Table {
        shared::table(lang)
    }

    /// Translate text, retaining unknown strings and English unchanged.
    pub fn t<'a>(&self, lang: Lang, text: &'a str) -> &'a str {
        if lang == Lang::En {
            return text;
        }
        self.maps[lang as usize]
            .get_or_init(|| {
                self.fallback_table(lang)
                    .iter()
                    .chain(self.table(lang))
                    .copied()
                    .collect()
            })
            .get(text)
            .copied()
            .unwrap_or(text)
    }

    /// Translate and fill sequential or numbered placeholders.
    pub fn tf(&self, lang: Lang, template: &str, args: &[&str]) -> String {
        fill(self.t(lang, template), args)
    }
}

/// Fill `{}` in order or `{0}`, `{1}` by position, retaining unresolved placeholders.
pub fn fill(template: &str, args: &[&str]) -> String {
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
mod tests {
    use super::*;
    use rstest::rstest;
    #[rstest]
    #[case("liked", &[], "liked")]
    #[case("press y to block @{}", &["猫🐈‍⬛.test"], "press y to block @猫🐈‍⬛.test")]
    #[case("{} of {}", &["1", "2"], "1 of 2")]
    #[case("{1} の {0}", &["a", "b"], "b の a")]
    #[case("a {} b", &[], "a {} b")]
    #[case("open { brace", &["x"], "open { brace")]
    fn a_template_is_filled_in_order_or_by_position(
        #[case] template: &str,
        #[case] args: &[&str],
        #[case] want: &str,
    ) {
        assert_eq!(fill(template, args), want);
    }

    #[test]
    fn catalogs_fall_back_without_changing_unknown_strings_or_english() {
        let catalog = Catalog::new(&[(Lang::Ja, &[("client text", "固有の文言")])]);
        assert_eq!(catalog.t(Lang::Ja, "client text"), "固有の文言");
        assert_eq!(catalog.t(Lang::Ja, "Search"), "検索");
        assert_eq!(catalog.t(Lang::En, "Search"), "Search");
        assert_eq!(catalog.t(Lang::Ja, "unknown {}"), "unknown {}");
        assert_eq!(catalog.tf(Lang::Ja, "Search: {}", &["猫"]), "検索: 猫");
    }

    #[test]
    fn all_language_metadata_round_trips_locales() {
        for lang in Lang::ALL {
            assert_eq!(
                Lang::from_code(&format!("{}_REGION.UTF-8", lang.code().to_uppercase())),
                Some(lang)
            );
            assert!(!lang.name().is_empty());
        }
    }

    #[cfg(feature = "test-support")]
    #[test]
    fn shared_tables_are_complete_including_the_hint_without_account_search() {
        let catalog = Catalog::new(&[]);
        let marked = catalog
            .fallback_table(Lang::Ja)
            .iter()
            .map(|(en, _)| (*en).to_owned())
            .collect();
        let problems =
            crate::test_support::i18n::validate_catalog(&marked, &catalog, &Lang::ALL, 5);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
