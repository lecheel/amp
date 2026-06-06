//--+ src/commands/buffer_list.rs
use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::Application;
use std::path::PathBuf;

pub fn open_under_cursor(app: &mut Application) -> Result {
    // Check if we are in the special buffer list buffer
    let is_list_buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .and_then(|b| b.path.as_ref())
        .map(|p| p.to_string_lossy() == "[Buffer List]")
        .unwrap_or(false);

    // If not, fall back to the default Enter behavior (symbol jump)
    if !is_list_buffer {
        return commands::application::switch_to_symbol_jump_mode(app);
    }

    // Read the current line under the cursor
    // FIX: map to owned String to avoid referencing temporary value
    let line = app
        .workspace
        .current_buffer
        .as_ref()
        .and_then(|b| b.data().lines().nth(b.cursor.line).map(|s| s.to_string()))
        .context("No line under cursor")?;

    // Parse the line. Format: "path/to/file [+]" or "[No Name] [+]"
    // split().next() always yields at least one item, so unwrap() is safe.
    let path_str = line.split(" [").next().unwrap().trim();

    if path_str == "[No Name]" {
        bail!("Cannot open unnamed buffer by path");
    }

    let target_path = PathBuf::from(path_str);

    // Find the buffer by cycling through the workspace
    let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
    loop {
        app.workspace.next_buffer();
        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            if buf.path.as_ref() == Some(&target_path) {
                break;
            }
        }
        if app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
            bail!("Couldn't find buffer for path: {}", path_str);
        }
    }

    commands::application::switch_to_normal_mode(app)?;
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}
