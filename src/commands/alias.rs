// commands/alias.rs
use crate::commands::{self, Result};
use crate::models::application::Application;

pub fn save_and_exit(app: &mut Application) -> Result {
    commands::buffer::save(app)?;
    commands::application::exit(app)
}

pub fn force_exit(app: &mut Application) -> Result {
    // Close buffer without confirmation, then exit
    if let Some(buf) = app.workspace.current_buffer.as_ref() {
        let _ = app.view.forget_buffer(buf);
    }
    app.workspace.close_current_buffer();
    commands::application::exit(app)
}
