//! The language a post is written in, for its `langs`: Bluesky filters
//! feeds and offers translation by it. It is guessed from the letters of
//! the text, since bsky asks nothing when posting.

use crate::i18n::Lang;

/// A family of letters, each written by one language or a few.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    Kana,
    Hangul,
    Han,
    Latin,
    Cyrillic,
    Greek,
    Arabic,
    Hebrew,
    Thai,
    Devanagari,
}

impl Script {
    fn of(c: char) -> Option<Script> {
        Some(match c {
            '\u{3040}'..='\u{30FF}' | '\u{31F0}'..='\u{31FF}' | '\u{FF66}'..='\u{FF9F}' => {
                Script::Kana
            }
            '\u{1100}'..='\u{11FF}' | '\u{3130}'..='\u{318F}' | '\u{AC00}'..='\u{D7AF}' => {
                Script::Hangul
            }
            '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2FA1F}' => Script::Han,
            'a'..='z' | 'A'..='Z' | '\u{C0}'..='\u{24F}' if c.is_alphabetic() => Script::Latin,
            '\u{400}'..='\u{4FF}' => Script::Cyrillic,
            '\u{370}'..='\u{3FF}' => Script::Greek,
            '\u{600}'..='\u{6FF}' => Script::Arabic,
            '\u{590}'..='\u{5FF}' => Script::Hebrew,
            '\u{E00}'..='\u{E7F}' => Script::Thai,
            '\u{900}'..='\u{97F}' => Script::Devanagari,
            _ => return None,
        })
    }
}

/// The language `text` is most likely in, as a BCP-47 code, or `None` when
/// it has no letters to tell by. Links, mentions and tags are not counted:
/// a URL's letters would outweigh a short post in another script.
///
/// Any kana makes it Japanese. Otherwise the script with the most letters
/// decides; where several languages share it, `ui` (the language bsky is
/// shown in) is taken when it is one of them, as the writer most likely
/// writes in it: Han is Japanese for a Japanese reader and Chinese
/// otherwise, Latin is English unless bsky is shown in another language
/// written with it.
pub fn guess(text: &str, ui: Lang) -> Option<&'static str> {
    let mut counts: Vec<(Script, usize)> = Vec::new();
    let words = text
        .split_whitespace()
        .filter(|w| !(w.contains("://") || w.starts_with('@') || w.starts_with('#')));
    for c in words.flat_map(str::chars) {
        let Some(s) = Script::of(c) else { continue };
        if s == Script::Kana {
            return Some("ja");
        }
        match counts.iter_mut().find(|(k, _)| *k == s) {
            Some((_, n)) => *n += 1,
            None => counts.push((s, 1)),
        }
    }
    // The first to reach the most wins a tie, so the result does not
    // depend on anything but the text.
    let (script, _) = counts.into_iter().rev().max_by_key(|(_, n)| *n)?;
    Some(match script {
        Script::Kana => "ja",
        Script::Hangul => "ko",
        Script::Han if ui == Lang::Ja => "ja",
        Script::Han => "zh",
        Script::Latin => match ui {
            Lang::Es | Lang::Fr | Lang::De | Lang::Pt => ui.code(),
            _ => "en",
        },
        Script::Cyrillic => "ru",
        Script::Greek => "el",
        Script::Arabic => "ar",
        Script::Hebrew => "he",
        Script::Thai => "th",
        Script::Devanagari => "hi",
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case::kana("今日は晴れ", Lang::En, Some("ja"))]
    #[case::kana_in_english("I love ラーメン so much", Lang::En, Some("ja"))]
    #[case::half_width_kana("ｶﾞｲｼﾞﾝ", Lang::En, Some("ja"))]
    #[case::hangul("안녕하세요", Lang::En, Some("ko"))]
    #[case::han_for_chinese("今天天气很好", Lang::En, Some("zh"))]
    #[case::han_for_japanese_reader("東京駅", Lang::Ja, Some("ja"))]
    #[case::english("Good morning", Lang::Ja, Some("en"))]
    #[case::spanish_reader("Buenos días", Lang::Es, Some("es"))]
    #[case::latin_for_russian_reader("Good morning", Lang::Ru, Some("en"))]
    #[case::cyrillic("Доброе утро", Lang::En, Some("ru"))]
    #[case::greek("Καλημέρα", Lang::En, Some("el"))]
    #[case::arabic("صباح الخير", Lang::En, Some("ar"))]
    #[case::thai("สวัสดี", Lang::En, Some("th"))]
    #[case::emoji_only("👨‍👩‍👧‍👦🇯🇵❤️ 1️⃣ 123", Lang::En, None)]
    #[case::empty("", Lang::Ja, None)]
    #[case::link_does_not_outweigh(
        "你好 https://example.com/some/long/path?query=value",
        Lang::En,
        Some("zh")
    )]
    #[case::mentions_and_tags("@alice.bsky.social #rustlang 你好", Lang::En, Some("zh"))]
    fn a_post_is_given_the_language_of_its_letters(
        #[case] text: &str,
        #[case] ui: Lang,
        #[case] want: Option<&str>,
    ) {
        assert_eq!(guess(text, ui), want);
    }
}
