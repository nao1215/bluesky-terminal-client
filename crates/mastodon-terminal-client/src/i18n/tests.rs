use std::path::Path;

use rstest::rstest;

use super::*;
use terminal_kit::test_support::i18n::{SourceRoot, marked_strings, validate_catalog};

#[test]
fn a_code_or_a_locale_names_a_language() {
    for lang in crate::i18n::ALL {
        assert_eq!(crate::i18n::from_code(lang.code()), Some(lang));
    }
    assert_eq!(crate::i18n::from_code("JA-jp"), Some(Lang::Ja));
    assert_eq!(crate::i18n::from_code("klingon"), None);
}

// The first of LC_ALL, LC_MESSAGES and LANG that is set wins; C, POSIX
// and a language mstdn does not speak are English.
#[rstest]
#[case(&[("LANG", "ja_JP.UTF-8")], Lang::Ja)]
#[case(&[("LC_ALL", "C.UTF-8"), ("LANG", "ja_JP.UTF-8")], Lang::En)]
#[case(&[("LC_ALL", ""), ("LC_MESSAGES", "ja"), ("LANG", "en_US")], Lang::Ja)]
#[case(&[("LANG", "fr_FR.UTF-8")], Lang::En)]
#[case(&[], Lang::En)]
fn the_locale_names_the_language(#[case] env: &[(&str, &str)], #[case] want: Lang) {
    let got = crate::i18n::from_locale(|k| {
        env.iter()
            .find(|(n, _)| *n == k)
            .map(|(_, v)| v.to_string())
    });
    assert_eq!(got, want);
}

#[test]
fn the_language_is_the_threads_own() {
    set(Lang::Ja);
    std::thread::spawn(|| assert_eq!(current(), Lang::En))
        .join()
        .unwrap();
    set(Lang::En);
}

#[test]
fn every_language_has_every_string_and_only_those() {
    let marked = marked_strings(
        Path::new(env!("CARGO_MANIFEST_DIR")),
        &[
            SourceRoot {
                path: "src",
                marks: &["n!(", "t(", "tf("],
            },
            SourceRoot {
                path: "../terminal-kit/src/files/view.rs",
                marks: &["translate("],
            },
        ],
        &["i18n", "tests", "tests.rs"],
    )
    .unwrap();
    let problems = validate_catalog(&marked, &CATALOG, &ALL, 5);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
