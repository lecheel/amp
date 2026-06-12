use crate::errors::*;
use crate::models::application::modes::JumpMode;
use crate::presenters::standard_status_line;
use crate::view::{Colors, View};
use git2::Repository;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    mode: &mut JumpMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let status_entries = standard_status_line("JUMP", Colors::Inverted, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    mode.reset_display();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, Some(mode))?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&status_entries);
    }
    presenter.set_cursor(None);
    presenter.present()?;
    Ok(())
}
