use crate::errors::*;
use crate::models::application::modes::SelectMode;
use crate::presenters::standard_status_line;
use crate::view::{Colors, CursorType, View};
use git2::Repository;
use scribe::buffer::Range;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    mode: &SelectMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let status_entries = standard_status_line("SELECT", Colors::SelectMode, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let selected_range = Range::new(mode.anchor, *buf.cursor.clone());
    let data = buf.data();
    presenter.print_buffer(
        buf,
        &data,
        &workspace.syntax_set,
        Some(&[selected_range]),
        None,
    )?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&status_entries);
    }
    presenter.set_cursor_type(CursorType::Bar);
    presenter.present()?;
    Ok(())
}
