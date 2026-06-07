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

    if !is_list_buffer {
        return commands::application::switch_to_symbol_jump_mode(app);
    }

    // Read the current line under the cursor
    let line = app
        .workspace
        .current_buffer
        .as_ref()
        .and_then(|b| b.data().lines().nth(b.cursor.line).map(|s| s.to_string()))
        .context("No line under cursor")?;

    let path_str = line.split(" [").next().unwrap().trim();

    if path_str == "[No Name]" {
        bail!("Cannot open unnamed buffer by path");
    }

    let target_path = PathBuf::from(path_str);

    // Remember the buffer list's ID so we can remove it later
    let list_buffer_id = app.workspace.current_buffer.as_ref().and_then(|b| b.id);

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

    // Clean up: close the buffer list buffer
    if let Some(list_id) = list_buffer_id {
        // Navigate back to the list buffer to close it
        let current_id = app.workspace.current_buffer.as_ref().map(|b| b.id);

        // Find and remove the list buffer
        let mut found = false;
        loop {
            let buf_id = app.workspace.current_buffer.as_ref().and_then(|b| b.id);
            if buf_id == Some(list_id) {
                // Found it — close it
                if let Some(buf) = app.workspace.current_buffer.as_ref() {
                    let _ = app.view.forget_buffer(buf);
                }
                app.workspace.close_current_buffer();
                app.buffer_registry.unregister(Some(list_id));
                found = true;
                break;
            }
            app.workspace.next_buffer();

            // Safety: if we've looped all the way around, stop
            if app.workspace.current_buffer.as_ref().map(|b| b.id) == current_id {
                break;
            }
        }

        // Navigate back to the target buffer
        if found {
            loop {
                if app
                    .workspace
                    .current_buffer
                    .as_ref()
                    .map(|b| b.path.as_ref())
                    == Some(Some(&target_path))
                {
                    break;
                }
                app.workspace.next_buffer();

                // Safety break
                if app.workspace.current_buffer.as_ref().map(|b| b.id) == current_id {
                    break;
                }
            }
        }
    }

    commands::application::switch_to_normal_mode(app)?;
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}
