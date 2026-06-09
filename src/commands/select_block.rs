use crate::commands::{self, Result};
use crate::errors::*;
use crate::input::Key;
use crate::models::application::modes::select_block::BlockInsertType;
use crate::models::application::{Application, ClipboardContent, Mode, ModeKey};
use scribe::buffer::{Position, Range};

pub fn switch_to_select_block_mode(app: &mut Application) -> Result {
    let position = *app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?
        .cursor;
    app.switch_to(ModeKey::SelectBlock);
    if let Mode::SelectBlock(ref mut mode) = app.mode {
        mode.reset(position);
    }
    Ok(())
}

pub fn block_insert(app: &mut Application) -> Result {
    enter_block_insert(app, BlockInsertType::Insert)
}

pub fn block_append(app: &mut Application) -> Result {
    enter_block_insert(app, BlockInsertType::Append)
}

fn enter_block_insert(app: &mut Application, insert_type: BlockInsertType) -> Result {
    let (anchor, cursor) = match app.mode {
        Mode::SelectBlock(ref mode) => {
            let cursor = *app
                .workspace
                .current_buffer
                .as_ref()
                .context(BUFFER_MISSING)?
                .cursor;
            (mode.anchor, cursor)
        }
        _ => bail!("Not in block select mode"),
    };

    let first_line = anchor.line.min(cursor.line);
    let insert_column = match insert_type {
        BlockInsertType::Insert => anchor.offset.min(cursor.offset),
        BlockInsertType::Append => anchor.offset.max(cursor.offset) + 1,
    };

    commands::buffer::start_command_group(app)?;

    let buf = app
        .workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?;

    // Pad the first line if needed (especially for append mode)
    {
        let data = buf.data();
        if let Some(line_content) = data.lines().nth(first_line) {
            let line_len = line_content.chars().count();
            if line_len < insert_column {
                drop(data);
                buf.cursor.move_to(Position {
                    line: first_line,
                    offset: line_len,
                });
                let padding = " ".repeat(insert_column - line_len);
                buf.insert(padding);
            }
        }
    }

    buf.cursor.move_to(Position {
        line: first_line,
        offset: insert_column,
    });

    app.switch_to(ModeKey::BlockInsert);
    if let Mode::BlockInsert(ref mut mode) = app.mode {
        mode.reset(anchor, cursor, insert_type);
    }

    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

pub fn insert_char(app: &mut Application) -> Result {
    commands::buffer::insert_char(app)?;
    if let Mode::BlockInsert(ref mut mode) = app.mode {
        if let Some(Key::Char(c)) = *app.view.last_key() {
            mode.typed_text.push(c);
        }
    }
    Ok(())
}

pub fn backspace(app: &mut Application) -> Result {
    if let Mode::BlockInsert(ref mut mode) = app.mode {
        if mode.typed_text.is_empty() {
            return Ok(());
        }
    }
    commands::buffer::backspace(app)?;
    if let Mode::BlockInsert(ref mut mode) = app.mode {
        mode.typed_text.pop();
    }
    Ok(())
}

pub fn insert_newline(app: &mut Application) -> Result {
    commands::buffer::insert_newline(app)
}

pub fn insert_tab(app: &mut Application) -> Result {
    commands::buffer::insert_tab(app)?;
    if let Mode::BlockInsert(ref mut mode) = app.mode {
        mode.typed_text.push('\t');
    }
    Ok(())
}

pub fn apply_and_exit(app: &mut Application) -> Result {
    let (typed_text, start_line, end_line, insert_column) = {
        if let Mode::BlockInsert(ref mode) = app.mode {
            (
                mode.typed_text.clone(),
                mode.start_line,
                mode.end_line,
                mode.insert_column,
            )
        } else {
            bail!("Not in block insert mode");
        }
    };

    if !typed_text.is_empty() && start_line < end_line {
        let buf = app
            .workspace
            .current_buffer
            .as_mut()
            .context(BUFFER_MISSING)?;

        for line in (start_line + 1)..=end_line {
            let line_len = {
                let data = buf.data();
                data.lines()
                    .nth(line)
                    .map(|l| l.chars().count())
                    .unwrap_or(0)
            };

            if line_len < insert_column {
                buf.cursor.move_to(Position {
                    line,
                    offset: line_len,
                });
                let padding = " ".repeat(insert_column - line_len);
                buf.insert(padding);
            }

            buf.cursor.move_to(Position {
                line,
                offset: insert_column,
            });
            buf.insert(typed_text.clone());
        }
    }

    commands::buffer::end_command_group(app)?;
    commands::application::switch_to_normal_mode(app)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn copy_block_to_clipboard(app: &mut Application) -> Result {
    let (anchor, cursor) = match app.mode {
        Mode::SelectBlock(ref mode) => {
            let cursor = *app
                .workspace
                .current_buffer
                .as_ref()
                .context(BUFFER_MISSING)?
                .cursor;
            (mode.anchor, cursor)
        }
        _ => bail!("Not in block select mode"),
    };

    let min_line = anchor.line.min(cursor.line);
    let max_line = anchor.line.max(cursor.line);
    let min_offset = anchor.offset.min(cursor.offset);
    let max_offset = anchor.offset.max(cursor.offset);

    let block_text = {
        let buf = app
            .workspace
            .current_buffer
            .as_ref()
            .context(BUFFER_MISSING)?;
        let data = buf.data();
        let mut text = String::new();
        for line_no in min_line..=max_line {
            if let Some(line) = data.lines().nth(line_no) {
                let chars: Vec<char> = line.chars().collect();
                let start = min_offset.min(chars.len());
                let end = max_offset.min(chars.len());
                if start < end {
                    let slice: String = chars[start..end].iter().collect();
                    text.push_str(&slice);
                }
            }
            if line_no < max_line {
                text.push('\n');
            }
        }
        text
    };

    app.clipboard
        .set_content(ClipboardContent::Block(block_text))?;
    Ok(())
}

/// Delete the rectangular block content from the buffer.
/// Returns (min_line, max_line, min_offset) for use by callers.
fn delete_block_content(app: &mut Application) -> anyhow::Result<(usize, usize, usize)> {
    let (anchor, cursor) = match app.mode {
        Mode::SelectBlock(ref mode) => {
            let cursor = *app
                .workspace
                .current_buffer
                .as_ref()
                .context(BUFFER_MISSING)?
                .cursor;
            (mode.anchor, cursor)
        }
        _ => bail!("Not in block select mode"),
    };

    let min_line = anchor.line.min(cursor.line);
    let max_line = anchor.line.max(cursor.line);
    let min_offset = anchor.offset.min(cursor.offset);
    let max_offset = anchor.offset.max(cursor.offset);

    let buf = app
        .workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?;

    for line in (min_line..=max_line).rev() {
        let line_len = {
            let data = buf.data();
            data.lines()
                .nth(line)
                .map(|l| l.chars().count())
                .unwrap_or(0)
        };
        if line_len <= min_offset {
            continue;
        }
        let delete_end = max_offset.min(line_len);
        let range = Range::new(
            Position {
                line,
                offset: min_offset,
            },
            Position {
                line,
                offset: delete_end,
            },
        );
        buf.delete_range(range);
    }

    buf.cursor.move_to(Position {
        line: min_line,
        offset: min_offset,
    });

    Ok((min_line, max_line, min_offset))
}

// ---------------------------------------------------------------------------
// Public block-selection editing commands
// ---------------------------------------------------------------------------

pub fn delete(app: &mut Application) -> Result {
    delete_block_content(app)?;
    commands::application::switch_to_normal_mode(app)?;
    commands::view::scroll_to_cursor(app)?;
    Ok(())
}

pub fn copy(app: &mut Application) -> Result {
    copy_block_to_clipboard(app)?;
    commands::application::switch_to_normal_mode(app)
}

pub fn copy_and_delete(app: &mut Application) -> Result {
    copy_block_to_clipboard(app)?;
    delete_block_content(app)?;
    commands::application::switch_to_normal_mode(app)?;
    commands::view::scroll_to_cursor(app)?;
    Ok(())
}

pub fn change(app: &mut Application) -> Result {
    copy_block_to_clipboard(app)?;
    let (min_line, max_line, min_offset) = delete_block_content(app)?;

    commands::buffer::start_command_group(app)?;

    app.switch_to(ModeKey::BlockInsert);
    if let Mode::BlockInsert(ref mut mode) = app.mode {
        mode.start_line = min_line;
        mode.end_line = max_line;
        mode.left_column = min_offset;
        mode.right_column = min_offset;
        mode.insert_column = min_offset;
        mode.insert_type = BlockInsertType::Insert;
        mode.typed_text.clear();
    }

    commands::view::scroll_to_cursor(app)?;
    Ok(())
}
