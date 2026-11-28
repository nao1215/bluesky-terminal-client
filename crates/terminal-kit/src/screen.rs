//! Taking the terminal over for a full-screen client and giving it back.
//!
//! [`init_terminal`] turns raw mode on and enters the alternate screen, with
//! the frames written through a [`FrameWriter`]; [`restore_terminal`] puts
//! all of it back, every step taken even after one fails.

use std::io::{self, Write};

use crossterm::event::DisableBracketedPaste;
use crossterm::execute;

/// The terminal a client draws on: a frame goes out whole.
pub type Term = ratatui::Terminal<ratatui::backend::CrosstermBackend<FrameWriter>>;

/// Raw mode and the alternate screen, as `ratatui::try_init` sets them up,
/// with the frames written through a [`FrameWriter`]. On a failure the
/// caller puts back what was set up, with [`restore_terminal`].
pub fn init_terminal() -> io::Result<Term> {
    crossterm::terminal::enable_raw_mode()?;
    execute!(io::stdout(), crossterm::terminal::EnterAlternateScreen)?;
    ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(FrameWriter::new(
        io::stdout(),
    )))
}

/// Put the terminal back as the client found it: bracketed paste off, raw
/// mode off, the main screen back. Every step is taken even after one
/// fails, and the error names each step that failed.
pub fn restore_terminal() -> Result<(), String> {
    restore_steps(&mut [
        ("turning bracketed paste off", &mut || {
            execute!(io::stdout(), DisableBracketedPaste)
        }),
        (
            "leaving raw mode",
            &mut crossterm::terminal::disable_raw_mode,
        ),
        ("leaving the alternate screen", &mut || {
            execute!(io::stdout(), crossterm::terminal::LeaveAlternateScreen)
        }),
    ])
}

/// Take every step, even after one fails: ratatui's `restore` stopped at a
/// failed step, and one that failed to leave raw mode left the alternate
/// screen up too. The failures, each named by its step, are returned
/// together.
fn restore_steps(steps: &mut [(&str, &mut dyn FnMut() -> io::Result<()>)]) -> Result<(), String> {
    let failed: Vec<String> = steps
        .iter_mut()
        .filter_map(|(what, step)| step().err().map(|e| format!("{what}: {e}")))
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(failed.join("; "))
    }
}

/// Write `sequence` to the terminal now, outside the frame being drawn.
pub fn write_now(sequence: &str) -> io::Result<()> {
    let mut out = io::stdout();
    out.write_all(sequence.as_bytes())?;
    out.flush()
}

/// Holds what a frame writes until the frame is flushed, then hands it to
/// the terminal in one write, marked as one synchronized update (mode 2026)
/// so a terminal that knows the mode shows the frame only once it is whole;
/// one that does not ignores the marks. Through stdout's own buffer a
/// frame went out 1 KB at a time, and the terminal could show the screen
/// half drawn: the old posts below the new ones while scrolling.
pub struct FrameWriter<W: io::Write = io::Stdout> {
    out: W,
    /// [`BEGIN_UPDATE`], then the frame written so far.
    frame: Vec<u8>,
}

/// Starts and ends a synchronized update.
const BEGIN_UPDATE: &[u8] = b"\x1b[?2026h";
const END_UPDATE: &[u8] = b"\x1b[?2026l";

impl<W: io::Write> FrameWriter<W> {
    /// A writer that sends each frame to `out` in one write.
    pub fn new(out: W) -> Self {
        let mut frame = Vec::with_capacity(64 * 1024);
        frame.extend_from_slice(BEGIN_UPDATE);
        Self { out, frame }
    }
}

impl<W: io::Write> io::Write for FrameWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.frame.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if self.frame.len() > BEGIN_UPDATE.len() {
            self.frame.extend_from_slice(END_UPDATE);
            let written = self.out.write_all(&self.frame);
            self.frame.truncate(BEGIN_UPDATE.len());
            written?;
        }
        self.out.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::{FrameWriter, restore_steps};
    use std::io::{self, Write};

    // ratatui's restore stopped at the first step that failed: raw mode
    // that could not be left kept the alternate screen up too, and the
    // failure went unreported. Every step is taken, and each failure named.
    #[test]
    fn every_restore_step_runs_after_one_fails_and_each_failure_is_named() {
        let mut ran = Vec::new();
        let result = {
            let ran = std::cell::RefCell::new(&mut ran);
            restore_steps(&mut [
                ("turning bracketed paste off", &mut || {
                    ran.borrow_mut().push("paste");
                    Err(io::Error::other("stdout is closed"))
                }),
                ("leaving raw mode", &mut || {
                    ran.borrow_mut().push("raw");
                    Ok(())
                }),
                ("leaving the alternate screen", &mut || {
                    ran.borrow_mut().push("screen");
                    Err(io::Error::other("stdout is closed"))
                }),
            ])
        };
        assert_eq!(ran, ["paste", "raw", "screen"]);
        assert_eq!(
            result.unwrap_err(),
            "turning bracketed paste off: stdout is closed; leaving the alternate screen: stdout is closed"
        );
        assert_eq!(restore_steps(&mut [("x", &mut || Ok(()))]), Ok(()));
    }

    /// Each write it is given, as the terminal would receive it.
    #[derive(Default, Clone)]
    struct Writes(std::rc::Rc<std::cell::RefCell<Vec<Vec<u8>>>>);

    impl Write for Writes {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.borrow_mut().push(buf.to_vec());
            Ok(buf.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_frame_reaches_the_terminal_in_one_write_as_one_update() {
        let writes = Writes::default();
        let mut w = FrameWriter::new(writes.clone());
        for part in ["\x1b[1;1H", "家族👨‍👩‍👧", &"x".repeat(5000)] {
            w.write_all(part.as_bytes()).unwrap();
        }
        assert!(
            writes.0.borrow().is_empty(),
            "nothing goes out before the frame ends"
        );
        w.flush().unwrap();
        let want = format!("\x1b[?2026h\x1b[1;1H家族👨‍👩‍👧{}\x1b[?2026l", "x".repeat(5000));
        assert_eq!(*writes.0.borrow(), [want.into_bytes()]);
        // The next frame starts empty; a flush with nothing drawn sends nothing.
        w.flush().unwrap();
        assert_eq!(writes.0.borrow().len(), 1, "an empty frame sends nothing");
        w.write_all(b"b").unwrap();
        w.flush().unwrap();
        assert_eq!(writes.0.borrow().len(), 2);
        assert_eq!(writes.0.borrow()[1], b"\x1b[?2026hb\x1b[?2026l");
    }

    /// Draws a screen of text through ratatui and counts the writes the
    /// terminal gets.
    #[test]
    fn a_drawn_screen_is_one_write() {
        use ratatui::backend::CrosstermBackend;
        use ratatui::widgets::Paragraph;
        let writes = Writes::default();
        let mut term = ratatui::Terminal::with_options(
            CrosstermBackend::new(FrameWriter::new(writes.clone())),
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Fixed(ratatui::layout::Rect::new(0, 0, 120, 50)),
            },
        )
        .unwrap();
        let text = "今日は山に登りました 🏔️ the view from the top 👨‍👩‍👧\n".repeat(50);
        term.draw(|f| f.render_widget(Paragraph::new(text.as_str()), f.area()))
            .unwrap();
        let writes = writes.0.borrow();
        assert_eq!(writes.len(), 1);
        assert!(writes[0].len() > 4096, "{} bytes", writes[0].len());
    }
}
