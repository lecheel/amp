use crate::errors::*;
use crate::presenters::{current_buffer_status_line_data, git_status_line_data};
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use git2::Repository;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    if let Some(buf) = workspace.current_buffer.as_ref() {
        let data = buf.data();
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;

        let entries = presenter.which_key_entries("pending_right_bracket");

        if let Some(e) = error {
            presenter.print_error(&e.to_string());
        } else {
            presenter.print_status_line(&[
                StatusLineData {
                    content: " ] ".to_string(),
                    style: Style::Default,
                    colors: Colors::Inverted,
                },
                buffer_status,
                git_status_line_data(repo, &buf.path),
            ]);
        }

        presenter.print_which_key_popup("]", &entries);
        presenter.set_cursor_type(CursorType::Block);
        presenter.present()?;
    }
    Ok(())
}
