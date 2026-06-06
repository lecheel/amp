use crate::errors::*;
use crate::models::application::modes::ExMode;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;

pub fn display(
    workspace: &mut Workspace,
    mode: &ExMode,
    view: &mut View,
    error: &Option<Error>,
) -> Result<()> {
    // Capture buffer data before creating the presenter so it lives long enough
    let data = workspace
        .current_buffer
        .as_ref()
        .map(|buf| buf.data())
        .unwrap_or_default();

    let mut presenter = view.build_presenter()?;

    if let Some(buf) = workspace.current_buffer.as_ref() {
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    }

    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        let mode_display = format!(" {} ", mode);
        let input_display = format!(" {}", mode.input);

        presenter.print_status_line(&[
            StatusLineData {
                content: mode_display,
                style: Style::Default,
                colors: Colors::Inverted,
            },
            StatusLineData {
                content: input_display,
                style: Style::Default,
                colors: Colors::Focused,
            },
        ]);
    }

    // Position cursor at the end of the input prompt
    let cursor_offset =
        format!(" {} ", mode).graphemes(true).count() + mode.input.graphemes(true).count();
    let cursor_line = presenter.height() - 1;
    presenter.set_cursor(Some(Position {
        line: cursor_line,
        offset: cursor_offset,
    }));

    presenter.set_cursor_type(CursorType::BlinkingBar);
    presenter.present()?;

    Ok(())
}
