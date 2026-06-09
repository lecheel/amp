use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::modes::SearchSelectMode;
use crate::models::application::{Application, Mode, ModeKey};

pub fn switch_to_file_picker_mode(app: &mut Application) -> Result {
    let config = app.preferences.borrow().search_select_config();
    app.switch_to(ModeKey::FilePicker);
    if let Mode::FilePicker(ref mut mode) = app.mode {
        mode.reset(&app.workspace.path, config);
    }
    commands::search_select::search(app)?;
    Ok(())
}

pub fn accept(app: &mut Application) -> Result {
    let selected_path = if let Mode::FilePicker(ref mut mode) = app.mode {
        let selection = mode.selection().context("No file selected")?;
        selection.0.clone()
    } else {
        bail!("Not in file picker mode");
    };

    if selected_path.is_dir() {
        // Navigate into directory
        if let Mode::FilePicker(ref mut mode) = app.mode {
            mode.current_dir = selected_path;
            mode.query().clear(); // <-- Changed from mode.input.clear()
            mode.reload();
            commands::search_select::search(app)?;
        }
        Ok(())
    } else {
        // Open file and close popup
        let syntax_definition = app
            .preferences
            .borrow()
            .syntax_definition_name(&selected_path)
            .and_then(|name| app.workspace.syntax_set.find_syntax_by_name(&name).cloned());

        app.workspace
            .open_buffer(&selected_path)
            .context("Couldn't open a buffer for the specified path.")?;
        let buffer = app.workspace.current_buffer.as_mut().unwrap();
        if syntax_definition.is_some() {
            buffer.syntax_definition = syntax_definition;
        }
        app.view.initialize_buffer(buffer)?;
        app.switch_to(ModeKey::Normal);
        commands::view::scroll_cursor_to_center(app).ok();
        Ok(())
    }
}

pub fn navigate_up(app: &mut Application) -> Result {
    if let Mode::FilePicker(ref mut mode) = app.mode {
        mode.navigate_up();
        commands::search_select::search(app)?;
    }
    Ok(())
}
