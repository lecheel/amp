use crate::errors::*;
use crate::models::application::modes::PendingLeaderMode;
use crate::presenters::standard_status_line;
use crate::view::{Colors, CursorType, View};
use git2::Repository;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    mode: &PendingLeaderMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let pressed: String = mode.keys.iter().map(|k| k.display()).collect();
    let label = if pressed.is_empty() {
        "LEADER".to_string()
    } else {
        format!("LEADER {}", pressed)
    };
    let status_entries = standard_status_line(&label, Colors::PinnedQuery, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    let entries = presenter.which_key_leader_entries(&mode.keys);
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&status_entries);
    }
    let popup_title = if pressed.is_empty() {
        "leader".to_string()
    } else {
        format!("leader: {}", pressed)
    };
    presenter.print_which_key_popup(&popup_title, &entries);
    presenter.set_cursor_type(CursorType::Block);
    presenter.present()?;
    Ok(())
}
