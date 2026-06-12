use crate::errors::*;
use crate::models::application::CompletionOrigin;
use crate::presenters::standard_status_line;
use crate::view::{Colors, CursorType, Style, View};
use git2::Repository;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let completion = view.completion.clone();
    let status_entries = standard_status_line("INSERT", Colors::Insert, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    if let Some(ref completion) = completion {
        if completion.origin == CompletionOrigin::BufferWords {
            if let Some(entry) = completion.selection() {
                let suffix = &entry.value[completion.prefix.len()..];
                if !suffix.is_empty() {
                    if let Some(anchor) = presenter.cursor_screen_position() {
                        let ghost_fg = crate::view::RGBColor(0x69, 0x71, 0x7A);
                        presenter.print(
                            &anchor,
                            Style::Italic,
                            Colors::CustomForeground(ghost_fg),
                            suffix,
                        );
                    }
                }
            }
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
        presenter.print_status_line(&status_entries);
    }
    presenter.set_cursor_type(CursorType::BlinkingBar);
    presenter.present()?;
    Ok(())
}
