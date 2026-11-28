//! CLI output primitives. Write errors, including BrokenPipe, remain errors.
use std::io::{self, Write};

/// Write text exactly as supplied, without adding a newline.
pub fn write_text(out: &mut dyn Write, text: &str) -> io::Result<()> {
    out.write_all(text.as_bytes())
}

/// Write an already serialized JSON value followed by one newline.
/// The caller supplies valid single-line JSON, including any required escaping.
pub fn write_json_line(out: &mut dyn Write, json: &str) -> io::Result<()> {
    writeln!(out, "{json}")
}

/// Make the first paragraph of a clap error one line, without its `error: ` prefix.
pub fn first_paragraph(msg: &str) -> String {
    let para: Vec<&str> = msg
        .lines()
        .take_while(|l| !l.trim().is_empty())
        .map(str::trim)
        .collect();
    let line = para.join(" ");
    line.strip_prefix("error: ").unwrap_or(&line).to_string()
}

/// Format the common error object from JSON-encoded message and hint strings.
/// The client serializes the strings; a missing hint is emitted as JSON null.
/// This keeps serialization policy and its dependency in the client.
pub fn error_json(message_json: &str, hint_json: Option<&str>, status: u8) -> String {
    format!(
        "{{\"error\":{message_json},\"hint\":{},\"status\":{status}}}",
        hint_json.unwrap_or("null")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_preserves_newlines_and_error_json_escaping() {
        let mut bytes = Vec::new();
        write_text(&mut bytes, "a\nb").unwrap();
        write_json_line(&mut bytes, r#"{"text":"猫\n\""}"#).unwrap();
        assert_eq!(bytes, "a\nb{\"text\":\"猫\\n\\\"\"}\n".as_bytes());
        assert_eq!(
            error_json(r#""a\n\"""#, None, 3),
            r#"{"error":"a\n\"","hint":null,"status":3}"#
        );
        assert_eq!(
            first_paragraph("error: missing arguments:\n  <FILE>\n\nUsage: app"),
            "missing arguments: <FILE>"
        );
    }

    #[test]
    fn neither_writer_turns_broken_pipe_into_success() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::ErrorKind::BrokenPipe.into())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        assert_eq!(
            write_text(&mut Closed, "text").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        assert_eq!(
            write_json_line(&mut Closed, "null").unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
    }
}
