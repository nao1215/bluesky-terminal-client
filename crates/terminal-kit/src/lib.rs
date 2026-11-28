//! Drawing helpers for terminal clients built on ratatui and crossterm.
//!
//! None of it knows what the client shows: it lays text out in terminal
//! cells, edits a line or a box of text, scales pictures to the cells they
//! are drawn in, reads key events (emoji included on Windows), puts text in
//! the terminal's clipboard, names the colors a screen is drawn in, checks
//! the terminal before the UI starts, takes the screen over and gives it
//! back, and waits for the writes still on their way when the user quits.
//!
//! - [`text`]: display width, wrapping and truncation by grapheme cluster.
//! - [`input`]: a text field whose cursor steps over whole graphemes.
//! - [`scale`]: fast scaling of a picture to the box it is drawn in.
//! - [`events`]: the terminal's input events.
//! - [`clipboard`]: the OSC 52 sequence that fills the terminal's clipboard.
//! - [`theme`]: color themes and the color depth of the terminal.
//! - [`terminal`]: whether the client runs in a terminal and which graphics
//!   protocol it speaks.
//! - [`screen`]: raw mode and the alternate screen, frames written whole,
//!   and the terminal put back.
//! - [`clock`]: timestamps as local time.
//! - [`inflight`]: the jobs on their way, waited for before quitting.

pub mod clipboard;
pub mod clock;
pub mod events;
pub mod inflight;
pub mod input;
pub mod scale;
pub mod screen;
pub mod terminal;
pub mod text;
pub mod theme;
