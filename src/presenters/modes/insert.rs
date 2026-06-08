use crate::errors::*;
use crate::models::application::CompletionOrigin;
use crate::presenters::current_buffer_status_line_data;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::Workspace;

pub fn display(workspace: &mut Workspace, view: &mut View, error: &Option<Error>) -> Result<()> {
    let completion = view.completion.clone();

    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;

    // ── unified completion (ghost text + popup) ──
    if let Some(ref completion) = completion {
        if completion.origin == CompletionOrigin::BufferWords {
            if let Some(entry) = completion.selection() {
                let suffix = &entry.value[completion.prefix.len()..];
                if !suffix.is_empty() {
                    if let Some(anchor) = presenter.cursor_screen_position() {
                        let ghost_fg = crate::view::RGBColor(0x69, 0x71, 0x7A); // Dim gray
                        presenter.print(
                            &anchor,
                            Style::Italic,
                            Colors::CustomForeground(ghost_fg),
                            suffix,
                        );
                    }
                }
            }

            // Show popup only if there are multiple candidates
            if completion.entries.len() > 1 {
                let anchor = presenter
                    .cursor_screen_position()
                    .unwrap_or(scribe::buffer::Position { line: 0, offset: 0 });
                presenter.print_completion_popup(completion, anchor);
            }
        }
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
