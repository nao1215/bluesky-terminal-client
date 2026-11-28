//! Confirmation shared by command-line writers. EOF and every answer other
//! than an explicit yes decline without performing an action.
use std::io::{self, BufRead, Write};

/// Ask once, accepting `y`, `Y`, or `yes`.
pub fn ask(input: &mut dyn BufRead, output: &mut dyn Write, question: &str) -> io::Result<bool> {
    output.write_all(question.as_bytes())?;
    output.flush()?;
    let mut answer = String::new();
    input.read_line(&mut answer)?;
    Ok(matches!(answer.trim(), "y" | "Y" | "yes"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn only_explicit_yes_confirms() {
        for (answer, expected) in [
            ("y\n", true),
            ("Y\n", true),
            ("yes\n", true),
            ("", false),
            ("n\n", false),
            ("\n", false),
        ] {
            let mut out = Vec::new();
            assert_eq!(
                super::ask(&mut answer.as_bytes(), &mut out, "Confirm? ").unwrap(),
                expected
            );
            assert_eq!(out, b"Confirm? ");
        }
    }
}
