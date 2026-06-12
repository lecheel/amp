use crate::commands::{self, Result};
use crate::errors::*;
use crate::input::Key;
use crate::models::application::{Application, RepeatableAction};

pub fn repeat_last_change(app: &mut Application) -> Result {
    let action = app.last_action.clone();
    let keys = app.last_insert_keys.clone();

    let is_change_action = matches!(
        action,
        Some(RepeatableAction::ChangeCurrentLine)
            | Some(RepeatableAction::ChangeToken)
            | Some(RepeatableAction::ChangeRestOfLine)
            | Some(RepeatableAction::InsertModeEntry)
    );

    match action {
        Some(RepeatableAction::DeleteCurrentLine) => commands::buffer::delete_current_line(app)?,
        Some(RepeatableAction::DeleteToken) => commands::buffer::delete_token(app)?,
        Some(RepeatableAction::DeleteRestOfLine) => commands::buffer::delete_rest_of_line(app)?,
        Some(RepeatableAction::DeleteAroundFunction) => commands::daf::delete_around_function(app)?,
        Some(RepeatableAction::ChangeCurrentLine) => commands::buffer::change_current_line(app)?,
        Some(RepeatableAction::ChangeToken) => commands::buffer::change_token(app)?,
        Some(RepeatableAction::ChangeRestOfLine) => commands::buffer::change_rest_of_line(app)?,
        Some(RepeatableAction::Paste) => commands::buffer::paste(app)?,
        Some(RepeatableAction::PasteAbove) => commands::buffer::paste_above(app)?,
        Some(RepeatableAction::IndentLine) => commands::buffer::indent_line(app)?,
        Some(RepeatableAction::OutdentLine) => commands::buffer::outdent_line(app)?,
        Some(RepeatableAction::ToggleLineComment) => commands::buffer::toggle_line_comment(app)?,
        Some(RepeatableAction::MergeNextLine) => commands::buffer::merge_next_line(app)?,
        Some(RepeatableAction::InsertModeEntry) => {
            commands::application::switch_to_insert_mode(app)?
        }
        None => bail!("No change to repeat"),
    }

    // If it was a change/insert action, replay the recorded keys and exit insert mode
    if is_change_action && !keys.is_empty() {
        app.replaying_change = true;
        for key in keys {
            app.view.last_key = Some(key);
            match app.view.last_key() {
                Some(Key::Char(_)) => commands::buffer::insert_char(app)?,
                Some(Key::Backspace) => commands::buffer::backspace(app)?,
                Some(Key::Delete) => commands::buffer::delete(app)?,
                Some(Key::Enter) => commands::buffer::insert_newline(app)?,
                Some(Key::Tab) => commands::buffer::insert_tab(app)?,
                _ => {}
            }
        }
        app.replaying_change = false;
        commands::application::switch_to_normal_mode(app)?;
    }

    Ok(())
}
