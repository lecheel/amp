use crate::errors::*;
use crate::presenters::current_buffer_status_line_data;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::Workspace;

pub fn display(workspace: &mut Workspace, view: &mut View, error: &Option<Error>) -> Result<()> {
    // Clone before the mutable borrow from build_presenter.
    let completion = view.completion.clone();

    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;

    // ── unified completion popup ──
    if let Some(ref completion) = completion {
        let anchor = presenter
            .cursor_screen_position()
            .unwrap_or(scribe::buffer::Position { line: 0, offset: 0 });
        presenter.print_completion_popup(completion, anchor);
    }

    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&[
            StatusLineData {
                content: " INSERT ".to_string(),
                style: Style::Default,
                colors: Colors::Insert,
            },
            buffer_status,
        ]);
    }

    presenter.set_cursor_type(CursorType::BlinkingBar);
    presenter.present()?;
    Ok(())
}
