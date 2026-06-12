use crate::errors::*;
use crate::presenters::standard_status_line;
use crate::view::{Colors, CursorType, Style, View};
use git2::Repository;
use scribe::buffer::Position;
use scribe::Workspace;

pub fn display(
    workspace: &mut Workspace,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let mode_colors = if workspace
        .current_buffer
        .as_ref()
        .map(|b| view.effective_modified(b))
        .unwrap_or(false)
    {
        Colors::Warning
    } else {
        Colors::Inverted
    };
    let status_entries = standard_status_line("NORMAL", mode_colors, workspace, view, repo);

    let mut presenter = view.build_presenter()?;
    if let Some(buf) = workspace.current_buffer.as_ref() {
        let data = buf.data();
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
        if let Some(e) = error {
            presenter.print_error(&e.to_string());
        } else {
            presenter.print_status_line(&status_entries);
        }
        presenter.set_cursor_type(CursorType::Block);
        presenter.present()?;
    } else {
        // splash screen...
        let content = [
            format!("Amp v{}", env!("CARGO_PKG_VERSION")),
            format!("Build revision {}", env!("BUILD_REVISION")),
            String::from("© 2015-2025 Jordan MacDonald"),
            String::from(" "),
            String::from("Press \"?\" to view quick start guide"),
        ];
        let line_count = content.len();
        let vertical_offset = line_count / 2;
        for (line_no, line) in content.iter().enumerate() {
            let position = Position {
                line: (presenter.height() / 2 + line_no).saturating_sub(vertical_offset),
                offset: (presenter.width() / 2).saturating_sub(line.chars().count() / 2),
            };
            presenter.print(&position, Style::Default, Colors::Default, line);
        }
        if let Some(e) = error {
            presenter.print_error(&e.to_string());
        }
        presenter.set_cursor(None);
        presenter.present()?;
    }
    Ok(())
}
