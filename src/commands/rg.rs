use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::{Application, BufferMetadata, BufferType};
use crate::util;
use scribe::buffer::Position;
use std::path::{Path, PathBuf};
use std::process::Command;

const RG_BUFFER_PATH: &str = "[Ripgrep Results]";

/// Run ripgrep with the given pattern and display results in a virtual buffer.
pub fn search(app: &mut Application, pattern: &str) -> Result {
    if pattern.is_empty() {
        bail!("No pattern specified for :rg");
    }

    let workspace_path = app.workspace.path.clone();

    // Run ripgrep with --vimgrep for consistent "file:line:col:text" output
    let output = Command::new("rg")
        .args([
            "--vimgrep",
            "--color",
            "never",
            "--no-heading",
            "--max-count",
            "500",
            pattern,
        ])
        .current_dir(&workspace_path)
        .output()
        .context("Failed to run ripgrep. Is rg installed?")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() && stdout.is_empty() {
        bail!("rg error: {}", stderr.trim());
    }

    let results = if stdout.is_empty() {
        format!("No matches found for: {}\n", pattern)
    } else {
        let mut content = format!("Searching for: {}\n\n", pattern);
        content.push_str(&stdout);
        if !content.ends_with('\n') {
            content.push('\n');
        }
        content
    };

    // Check if an rg buffer already exists
    let existing_id = find_rg_buffer_id(app);

    if let Some(id) = existing_id {
        // Navigate to the existing buffer
        while app.workspace.current_buffer.as_ref().and_then(|b| b.id) != Some(id) {
            app.workspace.next_buffer();
        }
        if let Some(buf) = app.workspace.current_buffer.as_mut() {
            buf.replace(results);
            buf.cursor.move_to(Position { line: 0, offset: 0 });
        }
    } else {
        let mut rg_buffer = scribe::Buffer::new();
        rg_buffer.path = Some(PathBuf::from(RG_BUFFER_PATH));
        rg_buffer.insert(results);
        rg_buffer.cursor.move_to(Position { line: 0, offset: 0 });

        util::add_buffer(rg_buffer, app)?;

        // Register as virtual AFTER add_buffer (which may reassign id)
        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            app.view.buffer_registry.register(
                buf.id,
                BufferMetadata {
                    buffer_type: BufferType::Virtual,
                },
            );
        }
    }
    commands::application::switch_to_normal_mode(app)?;
    Ok(())
}

/// Ripgrep the word/token under the cursor.
pub fn search_under_cursor(app: &mut Application) -> Result {
    let pattern = app
        .workspace
        .current_buffer
        .as_ref()
        .and_then(|buf| {
            let data = buf.data();
            let line = data.lines().nth(buf.cursor.line)?;
            let offset = buf.cursor.offset;

            // Extract word at cursor position
            extract_word_at(line, offset)
        })
        .context("No word under cursor")?;

    search(app, &pattern)
}

/// Open the file/line under the cursor in the rg results buffer.
pub fn open_under_cursor(app: &mut Application) -> Result {
    // Check if we're in an rg results buffer
    let is_rg_buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .and_then(|b| b.path.as_ref())
        .map(|p| p.to_string_lossy() == RG_BUFFER_PATH)
        .unwrap_or(false);

    if !is_rg_buffer {
        // Fall back to buffer_list or symbol jump
        return crate::commands::buffer_list::open_under_cursor(app);
    }

    let line = app
        .workspace
        .current_buffer
        .as_ref()
        .and_then(|b| b.data().lines().nth(b.cursor.line).map(|s| s.to_string()))
        .context("No line under cursor")?;

    // Skip header lines (search pattern line and blank line)
    if line.starts_with("Searching for:") || line.is_empty() || line.starts_with("No matches") {
        bail!("Not a result line");
    }

    // Parse vimgrep format: path/to/file:line:col:text
    let parts: Vec<&str> = line.splitn(4, ':').collect();
    if parts.len() < 3 {
        bail!("Could not parse result line");
    }

    let file_path = parts[0];
    let line_num: usize = parts[1].parse().context("Could not parse line number")?;

    // Make path absolute relative to workspace
    let absolute_path = if Path::new(file_path).is_absolute() {
        PathBuf::from(file_path)
    } else {
        app.workspace.path.join(file_path)
    };

    if !absolute_path.exists() {
        bail!("File not found: {}", absolute_path.display());
    }

    // Open the file
    crate::util::open_buffer(&absolute_path, app)?;

    // Move to the correct line
    if let Some(buf) = app.workspace.current_buffer.as_mut() {
        let target_line = line_num.saturating_sub(1); // rg is 1-indexed
        buf.cursor.move_to(Position {
            line: target_line,
            offset: 0,
        });
    }

    commands::view::scroll_to_cursor(app)?;

    Ok(())
}

// ── Helpers ──────────────────────────────────────────────
fn find_rg_buffer_id(app: &mut Application) -> Option<usize> {
    let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
    let mut first = true;
    loop {
        if !first && app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
            break;
        }
        first = false;

        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            let is_rg = buf
                .path
                .as_ref()
                .map(|p| p.to_string_lossy() == RG_BUFFER_PATH)
                .unwrap_or(false);

            if is_rg {
                return buf.id;
            }
        }
        app.workspace.next_buffer();
    }
    None
}

/// Extract the word at the given offset in a line.
/// A word is defined as a sequence of alphanumeric/underscore characters.
fn extract_word_at(line: &str, offset: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    if offset >= chars.len() {
        return None;
    }

    // If cursor is on a non-word char, try the char before it
    let start_offset = if is_word_char(chars[offset]) {
        offset
    } else if offset > 0 && is_word_char(chars[offset - 1]) {
        offset - 1
    } else {
        return None;
    };

    // Find word boundaries
    let mut word_start = start_offset;
    while word_start > 0 && is_word_char(chars[word_start - 1]) {
        word_start -= 1;
    }

    let mut word_end = start_offset + 1;
    while word_end < chars.len() && is_word_char(chars[word_end]) {
        word_end += 1;
    }

    let word: String = chars[word_start..word_end].iter().collect();
    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}
