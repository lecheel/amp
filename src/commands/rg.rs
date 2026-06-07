use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::{Application, BufferMetadata, BufferType};
use crate::util;
use scribe::buffer::Position;
use std::path::{Path, PathBuf};
use std::process::Command;
use syntect::parsing::{SyntaxDefinition, SyntaxSet};

const RG_BUFFER_PATH: &str = "[Ripgrep Results]";

// Embedded syntax definition for ripgrep grouped output.
// This scopes file paths as "strings", line/col numbers as "constants",
// and headers as "keywords" so they pick up standard theme colors automatically.
const RG_SYNTAX_YAML: &str = r#"
%YAML 1.2
---
name: Ripgrep Results
scope: text.ripgrep
contexts:
  main:
    - match: '^(\d+)(:)(\d+)(:)'
      captures:
        1: constant.numeric.line-number.ripgrep
        2: punctuation.separator.ripgrep
        3: constant.numeric.column-number.ripgrep
        4: punctuation.separator.ripgrep
    - match: '^(\d+)(:)'
      captures:
        1: constant.numeric.line-number.ripgrep
        2: punctuation.separator.ripgrep
    - match: '^(Searching for:|No matches found for:)'
      scope: keyword.other.ripgrep
    - match: '^\S+$'
      scope: string.unquoted.file-path.ripgrep
"#;

/// Ensures the custom syntax is loaded into the workspace SyntaxSet
/// and returns the SyntaxReference to attach to the buffer.
fn ensure_rg_syntax(app: &mut Application) -> anyhow::Result<syntect::parsing::SyntaxReference> {
    // Check if we've already added it
    if let Some(syn) = app
        .workspace
        .syntax_set
        .find_syntax_by_name("Ripgrep Results")
        .cloned()
    {
        return Ok(syn);
    }

    let syntax_def = SyntaxDefinition::load_from_str(RG_SYNTAX_YAML, true, None)
        .context("Failed to parse Ripgrep Results syntax definition")?;

    // Take the existing syntax set, add our new syntax, and put it back
    let mut builder =
        std::mem::replace(&mut app.workspace.syntax_set, SyntaxSet::new()).into_builder();
    builder.add(syntax_def);
    app.workspace.syntax_set = builder.build();

    // Now find the syntax reference in the updated set
    app.workspace
        .syntax_set
        .find_syntax_by_name("Ripgrep Results")
        .cloned()
        .context("Failed to find just-added Ripgrep syntax")
}

/// Run ripgrep with the given pattern and display results in a virtual buffer.
pub fn search(app: &mut Application, pattern: &str) -> Result {
    if pattern.is_empty() {
        bail!("No pattern specified for :rg");
    }

    let workspace_path = app.workspace.path.clone();

    // Use --heading for grouped output, --line-number and --column for precise jumps
    let output = Command::new("rg")
        .args([
            "--column",
            "--line-number",
            "--heading",
            "--color",
            "never",
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

        let syntax_ref = ensure_rg_syntax(app)?;
        if let Some(buf) = app.workspace.current_buffer.as_mut() {
            buf.replace(results);
            buf.cursor.move_to(Position { line: 0, offset: 0 });
            // Ensure syntax highlighting is re-applied if it was cleared
            if buf.syntax_definition.is_none() {
                buf.syntax_definition = Some(syntax_ref);
            }
        }
    } else {
        let mut rg_buffer = scribe::Buffer::new();
        rg_buffer.path = Some(PathBuf::from(RG_BUFFER_PATH));
        rg_buffer.insert(results);
        rg_buffer.cursor.move_to(Position { line: 0, offset: 0 });

        // Apply the custom syntax highlighting
        rg_buffer.syntax_definition = Some(ensure_rg_syntax(app)?);

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
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    let data = buffer.data();
    let current_line_idx = buffer.cursor.line;

    let current_line = data
        .lines()
        .nth(current_line_idx)
        .context("No line under cursor")?;

    // Skip header lines and empty lines
    if current_line.is_empty()
        || current_line.starts_with("Searching for:")
        || current_line.starts_with("No matches")
    {
        bail!("Not a result line");
    }

    // Parse the current match line: "line_num:col_num:text" or "line_num:text"
    let parts: Vec<&str> = current_line.splitn(3, ':').collect();
    if parts.len() < 2 {
        bail!("Could not parse result line");
    }

    let line_num: usize = parts[0].parse().context("Could not parse line number")?;
    let col_num: Option<usize> = if parts.len() == 3 {
        parts[1].parse().ok()
    } else {
        None
    };

    // Find the file path by scanning backwards for the heading line
    let mut file_path = None;
    for i in (0..current_line_idx).rev() {
        if let Some(prev_line) = data.lines().nth(i) {
            if prev_line.is_empty()
                || prev_line.starts_with("Searching for:")
                || prev_line.starts_with("No matches")
            {
                break;
            }

            let starts_with_digit = prev_line
                .trim_start()
                .chars()
                .next()
                .map_or(false, |c| c.is_ascii_digit());

            if !starts_with_digit {
                file_path = Some(prev_line.trim().to_string());
                break;
            }
        }
    }

    let file_path_str = file_path.context("Could not find file path for this result")?;

    let absolute_path = if Path::new(&file_path_str).is_absolute() {
        PathBuf::from(file_path_str)
    } else {
        app.workspace.path.join(&file_path_str)
    };

    if !absolute_path.exists() {
        bail!("File not found: {}", absolute_path.display());
    }

    // Open the file
    crate::util::open_buffer(&absolute_path, app)?;

    // Move to the correct line (and column if available)
    if let Some(buf) = app.workspace.current_buffer.as_mut() {
        let target_line = line_num.saturating_sub(1);
        let target_col = col_num.unwrap_or(1).saturating_sub(1);
        buf.cursor.move_to(Position {
            line: target_line,
            offset: target_col,
        });
    }

    // CHANGED: Center the screen on the result instead of just scrolling to it
    commands::view::scroll_cursor_to_center(app)?;

    Ok(())
}

/// Switch to the existing [Ripgrep Results] buffer.
pub fn switch_to_last_rg(app: &mut Application) -> Result {
    let found_id = find_rg_buffer_id(app);

    if found_id.is_some() {
        // find_rg_buffer_id already navigated the workspace to the rg buffer.
        Ok(())
    } else {
        bail!("No previous ripgrep results buffer found")
    }
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

/// Jump to the next ripgrep result and open the file.
pub fn next_result(app: &mut Application) -> Result {
    navigate_result(app, Direction::Forward)
}

/// Jump to the previous ripgrep result and open the file.
pub fn prev_result(app: &mut Application) -> Result {
    navigate_result(app, Direction::Backward)
}

enum Direction {
    Forward,
    Backward,
}

fn navigate_result(app: &mut Application, direction: Direction) -> Result {
    // Find the RG buffer. This leaves the workspace pointing at it if found.
    let rg_id = find_rg_buffer_id(app).context("No ripgrep results buffer found")?;

    // Ensure we are currently on the RG buffer
    while app.workspace.current_buffer.as_ref().and_then(|b| b.id) != Some(rg_id) {
        app.workspace.next_buffer();
    }

    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?;
    let data = buffer.data();
    let total_lines = data.lines().count();

    if total_lines == 0 {
        bail!("Ripgrep buffer is empty");
    }

    let mut current_line = buffer.cursor.line;
    let offset = buffer.cursor.offset;

    // Scan for the next valid result line
    for _ in 0..total_lines {
        current_line = match direction {
            Direction::Forward => {
                if current_line + 1 >= total_lines {
                    0
                } else {
                    current_line + 1
                }
            }
            Direction::Backward => {
                if current_line == 0 {
                    total_lines - 1
                } else {
                    current_line - 1
                }
            }
        };

        if let Some(line_text) = data.lines().nth(current_line) {
            if is_result_line(line_text) {
                // Move the cursor to the valid result line
                buffer.cursor.move_to(Position {
                    line: current_line,
                    offset,
                });
                break;
            }
        }
    }

    // Now that the cursor is on a valid result, open it
    open_under_cursor(app)
}

/// Checks if a line is an actual search result (e.g., "10:5:match")
/// and not a file heading, empty line, or header text.
fn is_result_line(line: &str) -> bool {
    let trimmed = line.trim_start();

    // Check if it starts with a digit
    let mut chars = trimmed.chars().peekable();
    if !chars.peek().map_or(false, |c| c.is_ascii_digit()) {
        return false;
    }

    // Consume all leading digits
    while chars.peek().map_or(false, |c| c.is_ascii_digit()) {
        chars.next();
    }

    // The next character MUST be a colon to be a line number.
    // This distinguishes "10:5:match" from "123filename.rs" (file paths)
    chars.peek() == Some(&':')
}
