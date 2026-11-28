//! Source marks and translation completeness checks.
use crate::i18n::{Catalog, Lang};
use std::{collections::BTreeSet, io, path::Path};

/// A path relative to the client's manifest, with the marks it uses.
pub struct SourceRoot<'a> {
    /// File or directory to scan.
    pub path: &'a str,
    /// Function or macro prefixes, including their opening parenthesis.
    pub marks: &'a [&'a str],
}

/// Collect marked strings, preserving the historical textual scanner rules.
/// Exclusions match directory or file names. Inline test modules are omitted.
pub fn marked_strings(
    manifest: &Path,
    roots: &[SourceRoot<'_>],
    exclusions: &[&str],
) -> io::Result<BTreeSet<String>> {
    let mut found = BTreeSet::new();
    for root in roots {
        let mut paths = vec![manifest.join(root.path)];
        while let Some(path) = paths.pop() {
            if exclusions.iter().any(|name| path.ends_with(name)) {
                continue;
            }
            if path.is_dir() {
                for entry in std::fs::read_dir(path)? {
                    paths.push(entry?.path());
                }
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(path)?;
                let text = text
                    .split("#[cfg(test)]\nmod tests {")
                    .next()
                    .unwrap_or_default();
                found.extend(literals_marked(text, root.marks));
            }
        }
    }
    Ok(found)
}

fn literals_marked(text: &str, marks: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    for mark in marks {
        let mut rest = text;
        while let Some(at) = rest.find(mark) {
            let before = rest[..at].chars().next_back();
            rest = &rest[at + mark.len()..];
            // The mark inside a longer name (`get(`, `println!(`) is not one.
            if before.is_some_and(|c| c.is_alphanumeric() || c == '_') {
                continue;
            }
            // The string may be on the next line, where rustfmt put it.
            let Some(after) = rest.trim_start().strip_prefix('"') else {
                continue;
            };
            rest = after;
            let mut s = String::new();
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' => match chars.next() {
                        Some('n') => s.push('\n'),
                        Some('t') => s.push('\t'),
                        Some('u') => {
                            let hex: String =
                                chars.by_ref().skip(1).take_while(|c| *c != '}').collect();
                            s.extend(u32::from_str_radix(&hex, 16).ok().and_then(char::from_u32));
                        }
                        Some(other) => s.push(other),
                        None => {}
                    },
                    c => s.push(c),
                }
            }
            out.push(s);
        }
    }
    out
}

fn placeholders(s: &str) -> Vec<String> {
    let mut out: Vec<String> = s
        .match_indices('{')
        .filter_map(|(i, _)| {
            let close = s[i..].find('}')?;
            Some(s[i..i + close + 1].to_string())
        })
        .collect();
    out.sort();
    out
}

/// Check duplicate, unused, missing, empty and mismatched-placeholder translations.
pub fn validate_catalog(
    marked: &BTreeSet<String>,
    catalog: &Catalog,
    languages: &[Lang],
    minimum: usize,
) -> Vec<String> {
    let mut problems = Vec::new();
    if marked.len() <= minimum {
        problems.push(format!("too few marked strings: {}", marked.len()));
    }
    for lang in languages.iter().copied().filter(|l| *l != Lang::En) {
        let table = catalog.table(lang);
        let mut seen = BTreeSet::new();
        for (en, tr) in table.iter().chain(
            catalog
                .fallback_table(lang)
                .iter()
                .filter(|(en, _)| marked.contains(*en)),
        ) {
            if !seen.insert(*en) {
                problems.push(format!("{}: twice: {en:?}", lang.code()));
            }
            if !marked.contains(*en) {
                problems.push(format!("{}: not in the source: {en:?}", lang.code()));
            }
            if tr.trim().is_empty() {
                problems.push(format!("{}: empty for {en:?}", lang.code()));
            }
            let (want, got) = (placeholders(en), placeholders(tr));
            let mut by_position: Vec<String> =
                (0..want.len()).map(|i| format!("{{{i}}}")).collect();
            by_position.sort();
            if got != want && got != by_position {
                problems.push(format!("{}: {{}} differ: {en:?} -> {tr:?}", lang.code()));
            }
        }
        for en in marked {
            if !seen.contains(en.as_str()) {
                problems.push(format!("{}: missing: {en:?}", lang.code()));
            }
        }
    }
    problems
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scanner_keeps_the_client_specific_test_exclusions_and_external_marks() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(
            dir.path().join("src/main.rs"),
            "t(\"shown\"); get(\"ignored\");\n#[cfg(test)]\nmod tests { t(\"test\"); }",
        )
        .unwrap();
        std::fs::write(dir.path().join("src/tests.rs"), "n!(\"separate test\")").unwrap();
        std::fs::write(dir.path().join("browser.rs"), "translate(\"browser\")").unwrap();
        let roots = [
            SourceRoot {
                path: "src",
                marks: &["n!(", "t("],
            },
            SourceRoot {
                path: "browser.rs",
                marks: &["translate("],
            },
        ];
        let with_tests = marked_strings(dir.path(), &roots, &[]).unwrap();
        let without_tests = marked_strings(dir.path(), &roots, &["tests.rs"]).unwrap();
        assert_eq!(
            with_tests,
            ["shown", "separate test", "browser"]
                .map(str::to_owned)
                .into()
        );
        assert_eq!(
            without_tests,
            ["shown", "browser"].map(str::to_owned).into()
        );
        assert_eq!(
            literals_marked(r#"tf("a\n\t\u{732b}\"")"#, &["tf("]),
            ["a\n\t猫\""]
        );
    }

    #[test]
    fn completeness_reports_missing_unused_duplicate_empty_and_invalid_placeholders() {
        let catalog = Catalog::new(&[(
            Lang::Ja,
            &[("hello {}", "空"), ("unused", " "), ("hello {}", "重複")],
        )]);
        let marked = ["hello {}", "missing"].map(str::to_owned).into();
        let problems = validate_catalog(&marked, &catalog, &[Lang::Ja], 0).join("\n");
        for kind in [
            "missing:",
            "not in the source:",
            "twice:",
            "empty for",
            "{} differ:",
        ] {
            assert!(problems.contains(kind), "{problems}");
        }
    }
}
