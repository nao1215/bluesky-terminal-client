//! Nostr UI catalog; the selected language remains in each App.
use terminal_kit::i18n::{Catalog, Lang};
mod ja;
pub(crate) static CATALOG: Catalog = Catalog::new(&[(Lang::Ja, ja::TABLE)]);

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use terminal_kit::test_support::i18n::{SourceRoot, marked_strings, validate_catalog};

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
        let problems = validate_catalog(&marked, &CATALOG, &[Lang::En, Lang::Ja], 100);
        assert!(problems.is_empty(), "{}", problems.join("\n"));
    }
}
