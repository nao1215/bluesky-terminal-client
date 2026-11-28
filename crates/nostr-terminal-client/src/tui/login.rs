//! First-run configuration editor. No signer or relay is contacted here.
use crate::{
    accounts,
    error::{Error, Result},
};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::{
    layout::{Constraint, Layout},
    widgets::{Block, Paragraph},
};
use std::time::Duration;
use terminal_kit::{events::Input, input::TextInput, screen};

pub(super) fn configure(account: Option<&str>) -> Result<bool> {
    let mut terminal = screen::init_terminal().map_err(super::terminal_error)?;
    let result = (|| {
        let language = terminal_kit::locale::language_from_locale(|key| std::env::var(key).ok());
        let ja = language == "ja";
        let mut text = TextInput::multi("{\"publicKey\":\"\",\"relays\":[],\"keyFile\":null}");
        let mut message = String::new();
        let mut input = Input::new().map_err(super::terminal_error)?;
        loop {
            terminal.draw(|frame|{
                let [body,hint]=Layout::vertical([Constraint::Min(1),Constraint::Length(2)]).areas(frame.area());
                let lines=text.layout(usize::from(body.width.saturating_sub(2))).lines.join("\n");
                frame.render_widget(Paragraph::new(lines).block(Block::bordered().title(if ja{"アカウント設定 JSON"}else{"Account configuration JSON"})),body);
                frame.render_widget(Paragraph::new(format!("{}\n{message}",if ja{"ctrl+s 保存 · esc 終了 · 鍵の内容ではなく keyFile のパスを指定"}else{"ctrl+s save · esc quit · use a keyFile path, never paste a private key"})),hint);
            }).map_err(super::terminal_error)?;
            match input
                .next(Duration::from_millis(30))
                .map_err(super::terminal_error)?
            {
                Some(Event::Key(key))
                    if key.code == KeyCode::Esc
                        || (key.code == KeyCode::Char('c')
                            && key.modifiers.contains(KeyModifiers::CONTROL)) =>
                {
                    return Ok(false);
                }
                Some(Event::Key(key))
                    if key.code == KeyCode::Char('s')
                        && key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    let value = text.text();
                    match serde_json::from_str(&value)
                        .map_err(Error::usage)
                        .and_then(|v| accounts::add(account, v))
                    {
                        Ok(()) => return Ok(true),
                        Err(e) => message = e.error,
                    }
                }
                Some(Event::Key(key)) => {
                    text.handle_key(key);
                }
                Some(Event::Paste(value)) => text.insert_str(&value),
                _ => (),
            }
        }
    })();
    let restored = screen::restore_terminal().map_err(super::terminal_error);
    match (result, restored) {
        (Err(e), _) | (_, Err(e)) => Err(e),
        (Ok(done), Ok(())) => Ok(done),
    }
}
