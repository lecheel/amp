use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::modes::buffer_list::BufferEntry;
use crate::models::application::modes::SearchSelectMode;
use crate::models::application::{Application, Mode, ModeKey};

pub fn switch_to_buffer_list_mode(app: &mut Application) -> Result {
    let mut entries = Vec::new();
    let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
    let mut first = true;
    loop {
        if !first && app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
            break;
        }
        first = false;
        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            entries.push(BufferEntry {
                path: buf.path.clone(),
                buffer_id: buf.id,
                modified: app.view.effective_modified(buf),
            });
        }
        app.workspace.next_buffer();
    }
    let config = app.preferences.borrow().search_select_config();
    app.switch_to(ModeKey::BufferList);
    if let Mode::BufferList(ref mut mode) = app.mode {
        mode.reset(entries, config);
    }
    commands::search_select::search(app)?;
    Ok(())
}

pub fn accept(app: &mut Application) -> Result {
    let selected = if let Mode::BufferList(ref mut mode) = app.mode {
        mode.selection().cloned()
    } else {
        bail!("Not in buffer list mode");
    };
    let entry = selected.context("No buffer selected")?;

    // Navigate to the buffer with the matching ID
    if let Some(target_id) = entry.buffer_id {
        let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
        loop {
            if app.workspace.current_buffer.as_ref().and_then(|b| b.id) == Some(target_id) {
                break;
            }
            app.workspace.next_buffer();
            if app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
                bail!("Couldn't find buffer with ID: {:?}", target_id);
            }
        }
    }

    app.switch_to(ModeKey::Normal);
    commands::view::scroll_cursor_to_center(app).ok();
    Ok(())
}
