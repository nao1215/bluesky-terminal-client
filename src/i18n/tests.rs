use std::collections::BTreeSet;
use std::path::Path;

use rstest::rstest;

use super::*;

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
fn a_code_or_a_locale_names_a_language() {
    for lang in Lang::ALL {
        assert_eq!(Lang::from_code(lang.code()), Some(lang));
    }
    assert_eq!(Lang::from_code("JA-jp"), Some(Lang::Ja));
    assert_eq!(Lang::from_code("klingon"), None);
}

#[test]
fn the_language_is_the_threads_own() {
    set(Lang::Ja);
    std::thread::spawn(|| assert_eq!(current(), Lang::En))
        .join()
        .unwrap();
    set(Lang::En);
}

/// Every string marked in the source, with `n!("...")`, `t("...")` or
/// `tf("...")`, unescaped as Rust reads it.
fn marked() -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut dirs = vec![root];
    while let Some(dir) = dirs.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                if !path.ends_with("i18n") {
                    dirs.push(path);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text = std::fs::read_to_string(&path).unwrap();
                // What a file's tests write is not shown to anyone.
                let text = text.split("#[cfg(test)]\nmod tests").next().unwrap();
                found.extend(literals_marked(text));
            }
        }
    }
    found
}

fn literals_marked(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for mark in ["n!(", "t(", "tf("] {
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

#[test]
fn every_language_has_every_string_and_only_those() {
    let marked = marked();
    assert!(marked.len() > 100, "{}", marked.len());
    let mut problems = Vec::new();
    for lang in Lang::ALL.into_iter().filter(|l| *l != Lang::En) {
        let table = lang.table();
        let mut seen = BTreeSet::new();
        for (en, tr) in table {
            if !seen.insert(*en) {
                problems.push(format!("{}: twice: {en:?}", lang.code()));
            }
            if !marked.contains(*en) {
                problems.push(format!("{}: not in the source: {en:?}", lang.code()));
            }
            if tr.trim().is_empty() {
                problems.push(format!("{}: empty for {en:?}", lang.code()));
            }
            // The arguments filled in are the same ones, in whatever order.
            let (want, got) = (placeholders(en), placeholders(tr));
            let positional = |p: &[String]| p.iter().filter(|p| *p != "{}").count();
            if want.len() != got.len() || (positional(&want) == 0 && want != got) {
                problems.push(format!("{}: {{}} differ: {en:?} -> {tr:?}", lang.code()));
            }
        }
        for en in &marked {
            if !seen.contains(en.as_str()) {
                problems.push(format!("{}: missing: {en:?}", lang.code()));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "{} problems:\n{}",
        problems.len(),
        problems.join("\n")
    );
}
