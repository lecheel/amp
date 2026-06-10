use crate::errors::*;
use crate::models::application::modes::PendingLeaderMode;
use crate::presenters::current_buffer_status_line_data;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    mode: &PendingLeaderMode,
    view: &mut View,
    error: &Option<Error>,
) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buffer_status = current_buffer_status_line_data(workspace);
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;

    // Build which-key entries from leader tree
    let entries = presenter.which_key_leader_entries(&mode.keys);

    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        let pressed: String = mode.keys.iter().map(|k| k.display()).collect();
        let title = if pressed.is_empty() {
            "LEADER".to_string()
        } else {
            format!("LEADER {}", pressed)
        };
        presenter.print_status_line(&[
            StatusLineData {
                content: format!(" {} ", title),
                style: Style::Default,
                colors: Colors::PinnedQuery,
            },
            buffer_status,
        ]);
    }

    let pressed: String = mode.keys.iter().map(|k| k.display()).collect();
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
