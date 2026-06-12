use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::RepeatableAction;
use crate::models::application::{Application, ClipboardContent};
use scribe::buffer::{Position, Range};

pub fn delete_around_function(app: &mut Application) -> Result {
    // Scope the immutable borrow so it ends before we need &mut
    let (open_pos, close_pos) = {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .context(BUFFER_MISSING)?;
        let data = buffer.data();
        let cursor_line = buffer.cursor.line;
        let cursor_offset = buffer.cursor.offset;
        find_enclosing_brace_pair(&data, cursor_line, cursor_offset)
            .context("No enclosing function body found")?
    };
    // buffer borrow is now released

    // Read data again for signature start lookup
    let sig_start_line = {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .context(BUFFER_MISSING)?;
        let data = buffer.data();
        find_signature_start(&data, open_pos.line)
    };

    let close_line_len = {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .context(BUFFER_MISSING)?;
        let data = buffer.data();
        data.lines()
            .nth(close_pos.line)
            .context(CURRENT_LINE_MISSING)?
            .len()
    };

    // Now safe to mutably borrow
    let start = Position {
        line: sig_start_line,
        offset: 0,
    };
    let end = Position {
        line: close_pos.line,
        offset: close_line_len,
    };
    let range = Range::new(start, end);

    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?;
    let content = buffer
        .read(&range)
        .context("Couldn't read function content")?;
    app.clipboard
        .set_content(ClipboardContent::Block(content))?;
    buffer.delete_range(range.clone());
    buffer.cursor.move_to(range.start());

    app.last_action = Some(RepeatableAction::DeleteAroundFunction);

    commands::view::scroll_to_cursor(app).context(SCROLL_TO_CURSOR_FAILED)
}

/// Find the innermost { } pair that contains the cursor position.
/// Search backward from cursor for a '{', then match forward to '}'.
/// If cursor is inside, return (open_pos, close_pos).
fn find_enclosing_brace_pair(
    data: &str,
    cursor_line: usize,
    cursor_offset: usize,
) -> Option<(Position, Position)> {
    let lines: Vec<&str> = data.lines().collect();
    let mut best_open: Option<Position> = None;

    // Search backward from cursor for opening braces
    for line_idx in (0..=cursor_line).rev() {
        let line = lines.get(line_idx)?;
        let start_offset = if line_idx == cursor_line {
            cursor_offset
        } else {
            line.len()
        };
        // Collect chars into a Vec so we can reverse the enumeration
        let chars: Vec<(usize, char)> = line.chars().enumerate().collect();
        for (offset, ch) in chars.into_iter().rev() {
            if offset > start_offset && line_idx == cursor_line {
                continue;
            }
            if ch == '{' {
                let open = Position {
                    line: line_idx,
                    offset,
                };
                if let Some(close) = match_bracket_forward(data, line_idx, offset) {
                    let cursor_before_close = close.line > cursor_line
                        || (close.line == cursor_line && close.offset >= cursor_offset);
                    let cursor_after_open = line_idx < cursor_line
                        || (line_idx == cursor_line && offset <= cursor_offset);
                    if cursor_before_close && cursor_after_open {
                        if best_open.is_none()
                            || line_idx > best_open.unwrap().line
                            || (line_idx == best_open.unwrap().line
                                && offset > best_open.unwrap().offset)
                        {
                            best_open = Some(open);
                        }
                    }
                }
            }
        }
    }

    best_open.and_then(|open| {
        match_bracket_forward(data, open.line, open.offset).map(|close| (open, close))
    })
}

fn match_bracket_forward(data: &str, start_line: usize, start_offset: usize) -> Option<Position> {
    let mut all_chars: Vec<(char, Position)> = Vec::new();
    for (line_idx, line) in data.lines().enumerate() {
        for (offset, ch) in line.chars().enumerate() {
            all_chars.push((
                ch,
                Position {
                    line: line_idx,
                    offset,
                },
            ));
        }
    }
    let start_idx = all_chars
        .iter()
        .position(|(ch, pos)| *ch == '{' && pos.line == start_line && pos.offset == start_offset)?;

    let mut depth = 1;
    for i in (start_idx + 1)..all_chars.len() {
        match all_chars[i].0 {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(all_chars[i].1);
                }
            }
            _ => {}
        }
    }
    None
}

/// Walk backward from the '{' line to find where the function signature starts.
/// Stops at the first line that looks like a declaration beginning.
fn find_signature_start(data: &str, brace_line: usize) -> usize {
    let lines: Vec<&str> = data.lines().collect();
    let mut sig_line = brace_line;

    // Walk backward from the line containing {.
    // A function signature might span multiple lines (e.g., in Rust/C).
    // Stop when we hit a blank line, a closing }, or the beginning of file.
    for line_idx in (0..brace_line).rev() {
        let line = lines.get(line_idx).map(|l| l.trim()).unwrap_or("");
        if line.is_empty() || line.starts_with('}') || line.starts_with('#') {
            break;
        }
        sig_line = line_idx;
    }

    sig_line
}
