use crate::errors::*;
use crate::view::View;
use scribe::Workspace;

pub fn display(workspace: &mut Workspace, view: &mut View, error: &Error) -> Result<()> {
    let data;
    let mut presenter = view.build_presenter()?;

    // Draw the buffer behind the popup
    if let Some(buffer) = workspace.current_buffer.as_ref() {
        data = buffer.data();
        let _ = presenter.print_buffer(buffer, &data, &workspace.syntax_set, None, None);
    }

    // Show the error in a square-cornered popup instead of the status line
    let error_string = error.to_string();
    let lines: Vec<&str> = error_string.lines().collect();
    presenter.print_error_popup("Error", &lines);

    presenter.present()?;
    Ok(())
}
