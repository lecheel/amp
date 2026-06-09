use crate::errors::*;
use crate::models::application::modes::select_block::{
    BlockInsertMode, BlockInsertType, SelectBlockMode,
};
use crate::presenters::current_buffer_status_line_data;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::Workspace;

pub fn display_select_block(
    workspace: &mut Workspace,
    mode: &SelectBlockMode,
    view: &mut View,
    error: &Option<Error>,
) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let cursor = *buf.cursor.clone();
    let ranges = mode.to_ranges(&cursor);
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, Some(&ranges), None)?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&[
            StatusLineData {
                content: " BLOCK ".to_string(),
                style: Style::Default,
                colors: Colors::SelectMode,
            },
            buffer_status,
        ]);
    }
    presenter.set_cursor_type(CursorType::Block);
    presenter.present()?;
    Ok(())
}

pub fn display_block_insert(
    workspace: &mut Workspace,
    mode: &BlockInsertMode,
    view: &mut View,
    error: &Option<Error>,
) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let ranges = mode.to_ranges();
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, Some(&ranges), None)?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        let mode_label = match mode.insert_type {
            BlockInsertType::Insert => " BLOCK INSERT ",
            BlockInsertType::Append => " BLOCK APPEND ",
        };
        presenter.print_status_line(&[
            StatusLineData {
                content: mode_label.to_string(),
                style: Style::Default,
                colors: Colors::Insert,
            },
            buffer_status,
        ]);
    }
    presenter.set_cursor_type(CursorType::Bar);
    presenter.present()?;
    Ok(())
}
