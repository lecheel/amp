use crate::errors::*;
use crate::presenters::current_buffer_status_line_data;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::Workspace;

pub fn display(workspace: &mut Workspace, view: &mut View, error: &Option<Error>) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;

    let entries = presenter.which_key_entries("pending_change");

    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&[
            StatusLineData {
                content: " CHANGE ".to_string(),
                style: Style::Default,
                colors: Colors::Insert,
            },
            buffer_status,
        ]);
    }

    presenter.print_which_key_popup("change", &entries);
    presenter.set_cursor_type(CursorType::Block);
    presenter.present()?;
    Ok(())
}
