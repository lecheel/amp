use crate::errors::*;
use crate::models::application::modes::select_block::{
    BlockInsertMode, BlockInsertType, SelectBlockMode,
};
use crate::presenters::standard_status_line;
use crate::view::{Colors, CursorType, View};
use git2::Repository;
use scribe::Workspace;

pub fn display_select_block(
    workspace: &mut Workspace,
    mode: &SelectBlockMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let status_entries = standard_status_line("BLOCK", Colors::SelectMode, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let cursor = *buf.cursor.clone();
    let ranges = mode.to_ranges(&cursor);
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, Some(&ranges), None)?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&status_entries);
    }
    presenter.set_cursor_type(CursorType::Block);
    presenter.present()?;
    Ok(())
}

pub fn display_block_insert(
    workspace: &mut Workspace,
    mode: &BlockInsertMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let mode_label = match mode.insert_type {
        BlockInsertType::Insert => "BLOCK INSERT",
        BlockInsertType::Append => "BLOCK APPEND",
    };
    let status_entries = standard_status_line(mode_label, Colors::Insert, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let ranges = mode.to_ranges();
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, Some(&ranges), None)?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&status_entries);
    }
    presenter.set_cursor_type(CursorType::Bar);
    presenter.present()?;
    Ok(())
}
