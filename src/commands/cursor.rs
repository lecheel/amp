use super::{application, buffer};
use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::git_gutter;
use crate::models::application::Application;
use crate::models::application::GitGutterStatus;
use crate::util::token::{adjacent_token_position, Direction};
use scribe::buffer::Position;

pub fn move_to_next_hunk(app: &mut Application) -> Result {
    let cursor_line = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?
        .cursor
        .line;

    let statuses = gutter_statuses(app)?;

    // If we're currently inside a hunk, skip past it first
    // so that repeated ]h presses cycle through hunks.
    let start_search =
        if cursor_line < statuses.len() && statuses[cursor_line] != GitGutterStatus::Unchanged {
            // Find the end of the current hunk
            let mut end = cursor_line + 1;
            while end < statuses.len() && statuses[end] != GitGutterStatus::Unchanged {
                end += 1;
            }
            end
        } else {
            cursor_line + 1
        };

    // Find the start of the next hunk (first non-unchanged line)
    let target_line = statuses
        .iter()
        .enumerate()
        .skip(start_search)
        .find(|(_, status)| **status != GitGutterStatus::Unchanged)
        .map(|(line, _)| line);

    if let Some(line) = target_line {
        let buffer = app
            .workspace
            .current_buffer
            .as_mut()
            .context(BUFFER_MISSING)?;
        let offset = buffer.cursor.offset;
        buffer.cursor.move_to(Position { line, offset });
    }

    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_previous_hunk(app: &mut Application) -> Result {
    let cursor_line = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?
        .cursor
        .line;

    let statuses = gutter_statuses(app)?;

    // If we're currently inside a hunk, skip before it first
    // so that repeated [h presses cycle through hunks.
    let start_search =
        if cursor_line < statuses.len() && statuses[cursor_line] != GitGutterStatus::Unchanged {
            // Find the start of the current hunk
            let mut start = cursor_line;
            while start > 0 && statuses[start] != GitGutterStatus::Unchanged {
                start -= 1;
            }
            if statuses[start] == GitGutterStatus::Unchanged {
                start
            } else {
                0
            }
        } else {
            cursor_line
        };

    // Find the start of the previous hunk by scanning backwards
    // to find the first non-unchanged line, then find the actual
    // start of that hunk.
    let target_line = statuses
        .iter()
        .enumerate()
        .take(start_search)
        .rfind(|(_, status)| **status != GitGutterStatus::Unchanged)
        .map(|(line, _)| {
            // Walk backwards to find the true start of this hunk
            let mut hunk_start = line;
            while hunk_start > 0 && statuses[hunk_start - 1] != GitGutterStatus::Unchanged {
                hunk_start -= 1;
            }
            hunk_start
        });

    if let Some(line) = target_line {
        let buffer = app
            .workspace
            .current_buffer
            .as_mut()
            .context(BUFFER_MISSING)?;
        let offset = buffer.cursor.offset;
        buffer.cursor.move_to(Position { line, offset });
    }

    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

fn gutter_statuses(app: &Application) -> anyhow::Result<Vec<GitGutterStatus>> {
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    let repo = app.repository.as_ref().context("No git repository found")?;
    git_gutter::line_statuses(repo, buffer)
}

pub fn move_up(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_up();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_down(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_down();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_left(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_left();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_right(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_right();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_start_of_line(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_to_start_of_line();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_end_of_line(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_to_end_of_line();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_first_line(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_to_first_line();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_last_line(app: &mut Application) -> Result {
    app.workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?
        .cursor
        .move_to_last_line();
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_first_word_of_line(app: &mut Application) -> Result {
    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        let data = buffer.data();
        let current_line = data
            .lines()
            .nth(buffer.cursor.line)
            .context(CURRENT_LINE_MISSING)?;

        // Find the offset of the first non-whitespace character.
        let all_blank = current_line.chars().enumerate().all(|(offset, character)| {
            if !character.is_whitespace() {
                // Move the cursor to this position.
                let new_cursor_position = Position {
                    line: buffer.cursor.line,
                    offset,
                };
                buffer.cursor.move_to(new_cursor_position);

                false
            } else {
                true
            }
        });

        if all_blank {
            bail!("No characters on the current line");
        }
    } else {
        bail!(BUFFER_MISSING);
    }

    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn insert_at_end_of_line(app: &mut Application) -> Result {
    move_to_end_of_line(app)?;
    application::switch_to_insert_mode(app)?;
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn insert_at_first_word_of_line(app: &mut Application) -> Result {
    move_to_first_word_of_line(app)?;
    application::switch_to_insert_mode(app)?;
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn insert_with_newline(app: &mut Application) -> Result {
    move_to_end_of_line(app)?;
    buffer::start_command_group(app)?;
    buffer::insert_newline(app)?;
    application::switch_to_insert_mode(app)?;
    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn insert_with_newline_above(app: &mut Application) -> Result {
    let current_line_number = app
        .workspace
        .current_buffer
        .as_mut()
        .map(|b| b.cursor.line)
        .context(BUFFER_MISSING)?;

    if current_line_number == 0 {
        buffer::start_command_group(app)?;
        move_to_start_of_line(app)?;
        buffer::insert_newline(app)?;
        move_up(app)?;
        move_to_end_of_line(app)?;
        application::switch_to_insert_mode(app)?;
        commands::view::scroll_to_cursor(app)?;
    } else {
        move_up(app)?;
        insert_with_newline(app)?;
    }

    Ok(())
}

pub fn move_to_start_of_previous_token(app: &mut Application) -> Result {
    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        let position = adjacent_token_position(buffer, false, Direction::Backward)
            .context("Couldn't find previous token")?;

        buffer.cursor.move_to(position);
    } else {
        bail!(BUFFER_MISSING);
    }
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_start_of_next_token(app: &mut Application) -> Result {
    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        let position = adjacent_token_position(buffer, false, Direction::Forward)
            .context("Couldn't find next token")?;

        buffer.cursor.move_to(position);
    } else {
        bail!(BUFFER_MISSING);
    }
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn move_to_end_of_current_token(app: &mut Application) -> Result {
    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
        let position = adjacent_token_position(buffer, true, Direction::Forward)
            .context("Couldn't find next token")?;

        buffer.cursor.move_to(position);
    } else {
        bail!(BUFFER_MISSING);
    }
    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

pub fn append_to_current_token(app: &mut Application) -> Result {
    move_to_end_of_current_token(app)?;
    application::switch_to_insert_mode(app)
}

#[cfg(test)]
mod tests {
    use crate::models::application::Application;
    use scribe::buffer::Position;
    use scribe::Buffer;

    #[test]
    fn move_to_first_word_of_line_works() {
        // Set up the application.
        let mut app = set_up_application("    amp");

        // Move to the end of the line.
        let position = Position { line: 0, offset: 7 };
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(position);

        // Call the command.
        super::move_to_first_word_of_line(&mut app).unwrap();

        // Ensure that the cursor is moved to the start of the first word.
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 0, offset: 4 }
        );
    }

    #[test]
    fn move_to_start_of_previous_token_works() {
        // Set up the application.
        let mut app = set_up_application("\namp editor");

        // Move past the first non-whitespace token.
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 2 });

        // Call the command.
        super::move_to_start_of_previous_token(&mut app).unwrap();

        // Ensure that the cursor is moved to the start of the previous word.
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 0 }
        );
    }

    #[test]
    fn move_to_start_of_previous_token_skips_whitespace() {
        // Set up the application.
        let mut app = set_up_application("\namp editor");

        // Move to the start of the second non-whitespace word.
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 4 });

        // Call the command.
        super::move_to_start_of_previous_token(&mut app).unwrap();

        // Ensure that the cursor is moved to the start of the previous word.
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 0 }
        );
    }

    #[test]
    fn move_to_start_of_next_token_works() {
        // Set up the application.
        let mut app = set_up_application("\namp editor");

        // Move to the start of the first non-whitespace word.
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 0 });

        // Call the command.
        super::move_to_start_of_next_token(&mut app).unwrap();

        // Ensure that the cursor is moved to the start of the next word.
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 4 }
        );
    }

    #[test]
    fn move_to_end_of_current_token_works() {
        // Set up the application and run the command.
        let mut app = set_up_application("\namp editor");

        // Move to the start of the first non-whitespace word.
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 0 });

        // Call the command.
        super::move_to_end_of_current_token(&mut app).unwrap();

        // Ensure that the cursor is moved to the end of the current word.
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 3 }
        );
    }

    #[test]
    fn append_to_current_token_works() {
        // Set up the application.
        let mut app = set_up_application("\namp editor");

        // Move to the start of the first non-whitespace word.
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 0 });

        // Call the command.
        super::append_to_current_token(&mut app).unwrap();

        // Ensure that the cursor is moved to the end of the current word.
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 3 }
        );

        // Ensure that we're in insert mode.
        assert!(match app.mode {
            crate::models::application::Mode::Insert => true,
            _ => false,
        });
    }

    #[test]
    fn insert_with_newline_above_finds_nearest_non_blank_indent() {
        // Set up the application.
        let mut app = set_up_application("    amp editor\n");

        // Move to the start of the first non-whitespace word.
        app.workspace
            .current_buffer
            .as_mut()
            .unwrap()
            .cursor
            .move_to(Position { line: 1, offset: 0 });

        // Call the command.
        super::insert_with_newline_above(&mut app).unwrap();

        // Ensure that a new line is inserted with indentation above.
        assert_eq!(
            &*app.workspace.current_buffer.as_ref().unwrap().data(),
            "    amp editor\n    \n"
        );

        // Ensure that the cursor is moved to the end of the indentation.
        assert_eq!(
            *app.workspace.current_buffer.as_ref().unwrap().cursor,
            Position { line: 1, offset: 4 }
        );

        // Ensure that we're in insert mode.
        assert!(match app.mode {
            crate::models::application::Mode::Insert => true,
            _ => false,
        });
    }

    fn set_up_application(content: &str) -> Application {
        let mut app = Application::new(&Vec::new()).unwrap();
        let mut buffer = Buffer::new();

        // Insert data with indentation and move to the end of the line.
        buffer.insert(content);

        // Now that we've set up the buffer, add it to the application.
        app.workspace.add_buffer(buffer);

        app
    }

    #[test]
    fn move_to_next_hunk_errors_without_repository() {
        let mut app = set_up_application("unchanged\nmodified\nunchanged2\n");
        let result = super::move_to_next_hunk(&mut app);
        assert!(result.is_err());
    }

    #[test]
    fn move_to_previous_hunk_errors_without_repository() {
        let mut app = set_up_application("unchanged\nmodified\nunchanged2\n");
        let result = super::move_to_previous_hunk(&mut app);
        assert!(result.is_err());
    }
}
