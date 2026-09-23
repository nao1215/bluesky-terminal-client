//! Putting text in the terminal's clipboard.
//!
//! OSC 52 asks the terminal itself to hold the text, so it works over ssh
//! and inside tmux (with `set-clipboard on`), where a clipboard program on
//! the machine bsky runs on would put it somewhere nobody can paste from.
//! A terminal that does not know the sequence ignores it.

/// The escape sequence that puts `text` in the terminal's clipboard.
pub fn osc52(text: &str) -> String {
    format!("\x1b]52;c;{}\x07", base64(text.as_bytes()))
}

/// Standard base64 with padding, which is what OSC 52 carries. Writing it
/// here keeps a dependency out of the tree for twenty lines of table lookup.
fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = u32::from(b[0]) << 16 | u32::from(b[1]) << 8 | u32::from(b[2]);
        let six = |shift: u32| ALPHABET[(n >> shift & 0x3f) as usize] as char;
        out.push(six(18));
        out.push(six(12));
        out.push(if chunk.len() > 1 { six(6) } else { '=' });
        out.push(if chunk.len() > 2 { six(0) } else { '=' });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("", "")]
    #[case("f", "Zg==")]
    #[case("fo", "Zm8=")]
    #[case("foo", "Zm9v")]
    #[case("foob", "Zm9vYg==")]
    #[case("fooba", "Zm9vYmE=")]
    #[case("foobar", "Zm9vYmFy")]
    #[case(
        "https://bsky.app/profile/did:plc:me/post/mine",
        "aHR0cHM6Ly9ic2t5LmFwcC9wcm9maWxlL2RpZDpwbGM6bWUvcG9zdC9taW5l"
    )]
    // A skin tone, CJK, and a URL: the bytes are encoded, not the characters.
    #[case(
        "👍🏽 家族 https://bsky.app/x",
        "8J+RjfCfj70g5a625pePIGh0dHBzOi8vYnNreS5hcHAveA=="
    )]
    fn base64_is_the_standard_alphabet_with_padding(#[case] text: &str, #[case] want: &str) {
        assert_eq!(base64(text.as_bytes()), want);
    }

    #[test]
    fn the_sequence_asks_the_terminal_to_hold_the_text() {
        assert_eq!(osc52("foo"), "\x1b]52;c;Zm9v\x07");
    }
}
