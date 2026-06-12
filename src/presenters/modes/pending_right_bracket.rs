use crate::errors::*;
use crate::presenters::standard_status_line;
use crate::view::{Colors, CursorType, View};
use git2::Repository;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let status_entries = standard_status_line("]", Colors::Inverted, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    if let Some(buf) = workspace.current_buffer.as_ref() {
        let data = buf.data();
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
        let entries = presenter.which_key_entries("pending_right_bracket");
        if let Some(e) = error {
            presenter.print_error(&e.to_string());
        } else {
            presenter.print_status_line(&status_entries);
        }
        presenter.print_which_key_popup("]", &entries);
        presenter.set_cursor_type(CursorType::Block);
        presenter.present()?;
    }
    Ok(())
}
