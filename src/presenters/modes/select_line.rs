use crate::errors::*;
use crate::models::application::modes::SelectLineMode;
use crate::presenters::standard_status_line;
use crate::view::{Colors, View};
use git2::Repository;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    mode: &SelectLineMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let status_entries =
        standard_status_line("SELECT LINE", Colors::SelectMode, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let selected_range = mode.to_range(&buf.cursor);
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
    presenter.present()?;
    Ok(())
}
