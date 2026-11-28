use std::path::Path;

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
        &[SourceRoot {
            path: "src",
            marks: &["n!(", "t(", "tf("],
        }],
        &["i18n", "tests"],
    )
    .unwrap();
    let problems = validate_catalog(&marked, &CATALOG, &ALL, 100);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
