use crate::errors::*;
use crate::models::application::modes::SearchMode;
use crate::presenters::standard_status_line;
use crate::view::{Alignment, Colors, CursorType, StatusLineData, Style, View};
use git2::Repository;
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;

pub fn display(
    workspace: &mut Workspace,
    mode: &SearchMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let mut status_entries =
        standard_status_line("SEARCH", Colors::SearchMode, workspace, view, repo);
    // Replace expand entry with search input, add results count
    let search_input = format!(" {}", mode.input.as_ref().unwrap_or(&String::new()));
    let result_display = if mode.insert {
        String::new()
    } else if let Some(ref results) = mode.results {
        if results.len() == 1 {
            String::from("1 match")
        } else {
            format!(
                "{} of {} matches",
                results.selected_index() + 1,
                results.len()
            )
        }
    } else {
        String::new()
    };
    // Replace filename with search input
    status_entries[2] = StatusLineData {
        content: search_input,
        style: Style::Default,
        colors: Colors::Focused,
        alignment: Alignment::Expand,
    };
    // Add result count at the end
    status_entries.push(StatusLineData {
        content: result_display,
        style: Style::Default,
        colors: Colors::Focused,
        alignment: Alignment::Right,
    });

    let cursor_offset = " SEARCH ".graphemes(true).count()
        + format!(" {}", mode.input.as_ref().unwrap_or(&String::new()))
            .graphemes(true)
            .count();

    let mut presenter = view.build_presenter()?;
    let buffer = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buffer.data();
    presenter.print_buffer(
        buffer,
        &data,
        &workspace.syntax_set,
        mode.results.as_ref().map(|r| r.as_slice()),
        None,
    )?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&status_entries);
    }
    if mode.insert {
        let cursor_line = presenter.height() - 1;
        presenter.set_cursor(Some(Position {
            line: cursor_line,
            offset: cursor_offset,
        }));
    }
    presenter.set_cursor_type(CursorType::BlinkingBar);
    presenter.present()?;
    Ok(())
}
