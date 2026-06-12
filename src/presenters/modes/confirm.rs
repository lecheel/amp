use crate::errors::*;
use crate::view::{Alignment, Colors, StatusLineData, Style, View};
use scribe::Workspace;

pub fn display(workspace: &mut Workspace, view: &mut View, error: &Option<Error>) -> Result<()> {
    let mut presenter = view.build_presenter()?;
    let buf = workspace.current_buffer.as_ref().context(BUFFER_MISSING)?;
    let data = buf.data();
    presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        let confirmation = "Are you sure? (y/n)".to_string();
        presenter.print_status_line(&[StatusLineData {
            content: confirmation,
            style: Style::Bold,
            colors: Colors::Warning,
            alignment: Alignment::Expand,
        }]);
    }
    presenter.present()?;
    Ok(())
}
