//! Language selection shared by the English/Japanese clients.

/// Resolve a language code or locale to `en` or `ja`.
pub fn language_code(code: &str) -> Option<&'static str> {
    match code
        .split(['_', '-', '.', '@'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "en" => Some("en"),
        "ja" => Some("ja"),
        _ => None,
    }
}

/// First nonempty LC_ALL, LC_MESSAGES or LANG, falling back to English.
pub fn language_from_locale(get: impl Fn(&str) -> Option<String>) -> &'static str {
    ["LC_ALL", "LC_MESSAGES", "LANG"]
        .into_iter()
        .find_map(|key| get(key).filter(|value| !value.is_empty()))
        .and_then(|value| language_code(&value))
        .unwrap_or("en")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn locale_priority_and_unknown_language_fallback() {
        for (all, messages, lang, expected) in [
            (None, None, None, "en"),
            (Some(""), Some("ja_JP.UTF-8"), Some("en_US"), "ja"),
            (Some("C"), Some("ja"), Some("ja"), "en"),
            (Some("EN-gb"), Some("ja"), None, "en"),
            (None, None, Some("JA_jp.UTF-8"), "ja"),
        ] {
            assert_eq!(
                language_from_locale(|key| match key {
                    "LC_ALL" => all,
                    "LC_MESSAGES" => messages,
                    "LANG" => lang,
                    _ => None,
                }
                .map(str::to_owned)),
                expected
            );
        }
    }
}
