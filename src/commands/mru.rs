use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::modes::SearchSelectMode;
use crate::models::application::{Application, Mode};
use crate::util;

pub fn accept(app: &mut Application) -> Result {
    let target_path = if let Mode::MRU(ref mode) = app.mode {
        mode.selection().map(|s| s.0.clone())
    } else {
        bail!("Not in MRU mode");
    };

    if let Some(target_path) = target_path {
        if target_path.to_string_lossy() == "[No Name]" {
            bail!("Cannot open unnamed buffer by path");
        }
        if !target_path.exists() {
            bail!("File not found: {}", target_path.display());
        }

        // Check if the buffer is already open in the workspace
        let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
        let mut found = false;

        if start_id.is_some() {
            loop {
                app.workspace.next_buffer();
                if let Some(buf) = app.workspace.current_buffer.as_ref() {
                    if buf.path.as_ref() == Some(&target_path) {
                        found = true;
                        break;
                    }
                }
                if app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
                    break;
                }
            }
        }

        // If it wasn't already open, open it properly (restores cursor position)
        if !found {
            util::open_buffer(&target_path, app)?;
        }
    }

    commands::application::switch_to_normal_mode(app)?;
    Ok(())
}
