use crate::commands::{self, Result};
use crate::errors::*;
use crate::input::Key;
use crate::models::application::{Application, CompletionOrigin, CompletionState, Mode};

// ── public commands ──────────────────────────────────────────────

/// Alt-/ unified completion from buffer words.
pub fn complete_from_buffer(app: &mut Application) -> Result {
    let buffer_data = app
        .workspace
        .current_buffer
        .as_ref()
        .map(|b| b.data())
        .unwrap_or_default();

    match app.mode {
        Mode::Insert => complete_insert(app, &buffer_data),
        Mode::Ex(_) => complete_ex(app, &buffer_data),
        _ => Ok(()),
    }
}

pub fn select_next(app: &mut Application) -> Result {
    if let Some(ref mut c) = app.view.completion {
        c.select_next();
    }
    Ok(())
}

pub fn select_previous(app: &mut Application) -> Result {
    if let Some(ref mut c) = app.view.completion {
        c.select_previous();
    }
    Ok(())
}

pub fn accept(app: &mut Application) -> Result {
    let completion = app.view.completion.take();

    if let Some(c) = completion {
        if let Some(entry) = c.selection() {
            let value = entry.value.clone();
            let prefix_len = c.prefix.len();

            match c.origin {
                CompletionOrigin::BufferWords => {
                    let suffix = &value[prefix_len..];
                    if let Some(buffer) = app.workspace.current_buffer.as_mut() {
                        if !suffix.is_empty() {
                            buffer.insert(suffix.to_string());
                            if !app.replaying_change {
                                for c in suffix.chars() {
                                    app.current_insert_keys.push(Key::Char(c));
                                }
                            }
                            for _ in 0..suffix.chars().count() {
                                buffer.cursor.move_right();
                            }
                        }
                    }
                    commands::view::scroll_to_cursor(app)?;
                }
                CompletionOrigin::ExInput => {
                    if let Mode::Ex(ref mut mode) = app.mode {
                        let input = mode.input.trim_start_matches(':');
                        let mut prefix_start = input.len();
                        for (i, ch) in input.char_indices().rev() {
                            if ch.is_alphanumeric() || ch == '_' {
                                prefix_start = i;
                            } else {
                                break;
                            }
                        }
                        let before = &input[..prefix_start];
                        mode.input = format!(":{}{}", before, value);
                        mode.completions.clear();
                        mode.completion_selection = None;
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn cancel(app: &mut Application) -> Result {
    app.view.completion = None;
    Ok(())
}

/// Called before the mode keymap is consulted.
/// Returns `true` when the key was consumed by the completion popup.
pub fn handle_key(app: &mut Application) -> anyhow::Result<bool> {
    if app.view.completion.is_none() {
        return Ok(false);
    }

    let key = app.view.last_key();
    match *key {
        Some(Key::Down) | Some(Key::Tab) => {
            select_next(app)?;
            Ok(true)
        }
        Some(Key::Up) => {
            select_previous(app)?;
            Ok(true)
        }
        Some(Key::Enter) => {
            accept(app)?;
            Ok(true)
        }
        Some(Key::Esc) => {
            cancel(app)?;
            Ok(true)
        }
        _ => {
            app.view.completion = None;
            Ok(false)
        }
    }
}

// ── private helpers ──────────────────────────────────────────────

fn complete_insert(app: &mut Application, buffer_data: &str) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    let data = buffer.data();
    let current_line = match data.lines().nth(buffer.cursor.line) {
        Some(l) => l,
        None => {
            app.view.completion = None;
            return Ok(());
        }
    };

    let chars: Vec<char> = current_line.chars().collect();
    let char_offset = buffer.cursor.offset.min(chars.len());

    let prefix_end = char_offset;
    let mut prefix_start = prefix_end;
    while prefix_start > 0
        && (chars[prefix_start - 1].is_alphanumeric() || chars[prefix_start - 1] == '_')
    {
        prefix_start -= 1;
    }

    if prefix_start == prefix_end {
        app.view.completion = None;
        return Ok(());
    }

    let prefix: String = chars[prefix_start..prefix_end].iter().collect();

    match CompletionState::from_buffer_words(buffer_data, &prefix, CompletionOrigin::BufferWords) {
        Some(state) if state.entries.len() == 1 => {
            let value = state.entries[0].value.clone();
            let suffix = &value[prefix.len()..];
            if let Some(buf) = app.workspace.current_buffer.as_mut() {
                if !suffix.is_empty() {
                    buf.insert(suffix.to_string());
                    for _ in 0..suffix.chars().count() {
                        buf.cursor.move_right();
                    }
                }
            }
            commands::view::scroll_to_cursor(app)?;
            app.view.completion = None;
        }
        Some(state) => {
            app.view.completion = Some(state);
        }
        None => {
            app.view.completion = None;
        }
    }

    Ok(())
}

fn complete_ex(app: &mut Application, buffer_data: &str) -> Result {
    // Extract values from the mode first to avoid holding an immutable borrow
    // across the later mutable borrow.
    let (prefix, before) = if let Mode::Ex(ref mode) = app.mode {
        let input = mode.input.trim_start_matches(':');
        let mut prefix_start = input.len();
        for (i, c) in input.char_indices().rev() {
            if c.is_alphanumeric() || c == '_' {
                prefix_start = i;
            } else {
                break;
            }
        }
        (
            input[prefix_start..].to_string(),
            input[..prefix_start].to_string(),
        )
    } else {
        return Ok(());
    };

    if prefix.is_empty() {
        app.view.completion = None;
        return Ok(());
    }

    // Clear ex-specific completions so the two popup systems never overlap.
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.completions.clear();
        mode.completion_selection = None;
    }

    match CompletionState::from_buffer_words(&buffer_data, &prefix, CompletionOrigin::ExInput) {
        Some(state) if state.entries.len() == 1 => {
            let value = state.entries[0].value.clone();
            if let Mode::Ex(ref mut mode) = app.mode {
                mode.input = format!(":{}{}", before, value);
            }
            app.view.completion = None;
        }
        Some(state) => {
            app.view.completion = Some(state);
        }
        None => {
            app.view.completion = None;
        }
    }

    Ok(())
}
