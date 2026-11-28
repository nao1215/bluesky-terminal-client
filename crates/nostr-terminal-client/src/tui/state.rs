//! Persist the current account's read state and pinned columns.
use super::app::{App, List};
use crate::{error::Result, read_state::ReadState};
pub(super) fn load(app: &mut App) -> Result<()> {
    let state = ReadState::load(&app.public)?;
    app.extra.seen = state.seen;
    app.extra.columns = state.columns;
    app.extra.column_lists = app.extra.columns.iter().map(|_| List::default()).collect();
    Ok(())
}
pub(super) fn save(app: &App) -> Result<()> {
    let mut seen = ReadState::load(&app.public)?.seen;
    seen.extend(app.extra.seen.iter().cloned());
    ReadState {
        seen,
        columns: app.extra.columns.clone(),
    }
    .save(&app.public)
}
