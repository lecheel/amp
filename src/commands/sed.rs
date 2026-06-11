use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::{Application, BufferMetadata, BufferType, SedChange};
use crate::util;
use scribe::buffer::Position;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use syntect::parsing::{SyntaxDefinition, SyntaxSet};

const SED_BUFFER_PATH: &str = "[Sed Diff]";

const SED_DIFF_SYNTAX_YAML: &str = r#"
%YAML 1.2
---
name: Sed Diff
scope: source.diff.sed
contexts:
  main:
    - match: '^\[y\]'
      scope: markup.inserted.diff
    - match: '^\[ \]'
      scope: markup.deleted.diff
    - match: '⚠'
      scope: markup.deleted.diff
    - match: '^---'
      scope: meta.diff.header.from-file
    - match: '^\+\+\+'
      scope: meta.diff.header.to-file
    - match: '^@@'
      scope: meta.diff.range.unified
    - match: '^-'
      scope: markup.deleted.diff
    - match: '^\+'
      scope: markup.inserted.diff
    - match: '^(Sed:|n:|Review)'
      scope: comment.line
"#;

fn ensure_diff_syntax(app: &mut Application) -> anyhow::Result<syntect::parsing::SyntaxReference> {
    if let Some(syn) = app
        .workspace
        .syntax_set
        .find_syntax_by_name("Sed Diff")
        .cloned()
    {
        return Ok(syn);
    }
    let syntax_def = SyntaxDefinition::load_from_str(SED_DIFF_SYNTAX_YAML, true, None)
        .context("Failed to parse Sed Diff syntax definition")?;
    let mut builder =
        std::mem::replace(&mut app.workspace.syntax_set, SyntaxSet::new()).into_builder();
    builder.add(syntax_def);
    app.workspace.syntax_set = builder.build();
    app.workspace
        .syntax_set
        .find_syntax_by_name("Sed Diff")
        .cloned()
        .context("Failed to find just-added Sed Diff syntax")
}

/// Build the display buffer content from current sed_changes state
fn build_buffer_content(app: &Application) -> (String, usize) {
    let mut content = String::new();

    if let Some(first) = app.sed_changes.first() {
        content.push_str(&format!(
            "Sed: '{}' -> '{}'\n",
            first.old_text, first.new_text
        ));
    } else {
        content.push_str("Sed: (no changes)\n");
    }
    content.push_str("<SPC>: exclude  a: toggle all  w: apply  q: cancel\n\n");

    let header_lines = 3;

    let mut current_file: Option<&PathBuf> = None;
    for (idx, change) in app.sed_changes.iter().enumerate() {
        if current_file != Some(&change.file) {
            if current_file.is_some() {
                content.push('\n');
            }
            content.push_str(&format!("{}:\n", change.file.display()));
            current_file = Some(&change.file);
        }

        let marker = if change.confirmed { "[y]" } else { "[ ]" };
        let boundary_flag = if !change.word_boundary { " ⚠" } else { "" };

        content.push_str(&format!(
            "{} #{} L{}:C{}{}  {}\n",
            marker,
            idx,
            change.line + 1,
            change.column + 1,
            boundary_flag,
            change.context_line,
        ));
    }

    (content, header_lines)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Check if a match at byte_start..byte_end in content is a word-boundary match
fn is_word_boundary_match(content: &str, byte_start: usize, byte_end: usize) -> bool {
    let before_is_boundary = if byte_start == 0 {
        true
    } else {
        // Find the char whose last byte is byte_start - 1
        content[..byte_start]
            .chars()
            .next_back()
            .map_or(true, |c| !is_word_char(c))
    };

    let after_is_boundary = if byte_end >= content.len() {
        true
    } else {
        content[byte_end..]
            .chars()
            .next()
            .map_or(true, |c| !is_word_char(c))
    };

    before_is_boundary && after_is_boundary
}

fn find_all_occurrences(content: &str, pattern: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut search_from = 0;
    while let Some(pos) = content[search_from..].find(pattern) {
        let byte_start = search_from + pos;
        let byte_end = byte_start + pattern.len();
        results.push((byte_start, byte_end));
        search_from = byte_end;
    }
    results
}

fn line_col_for_offset(content: &str, byte_offset: usize) -> (usize, usize) {
    let before = &content[..byte_offset];
    let line = before.matches('\n').count();
    let col = byte_offset - before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    (line, col)
}

/// Extract the source line at the given line index, trimmed for display
fn extract_context_line(content: &str, line_idx: usize) -> String {
    let line = content.lines().nth(line_idx).unwrap_or("");
    let trimmed = line.trim();
    // Truncate if very long, keeping the match visible
    if trimmed.len() > 80 {
        trimmed[..trimmed.ceil_char_boundary(77)].to_string() + "..."
    } else {
        trimmed.to_string()
    }
}

/// Parse arguments, supporting optional -w flag
fn parse_args(args: &str) -> anyhow::Result<(bool, &str, &str, &str)> {
    let tokens: Vec<&str> = args.split_whitespace().collect();
    let mut word_only = false;
    let mut start = 0;

    if tokens.first() == Some(&"-w") {
        word_only = true;
        start = 1;
    }

    let old_pattern = tokens.get(start).copied().unwrap_or("");
    let new_pattern = tokens.get(start + 1).copied().unwrap_or("");
    let glob_pattern = tokens.get(start + 2).copied().unwrap_or("");

    if old_pattern.is_empty() {
        bail!("Usage: :sed [-w] old new [glob]");
    }

    Ok((word_only, old_pattern, new_pattern, glob_pattern))
}

pub fn run(app: &mut Application, args: &str) -> Result {
    let (word_only, old_pattern, new_pattern, glob_pattern) = parse_args(args)?;

    let workspace_path = app.workspace.path.clone();

    let mut rg_args = vec!["-l".to_string()];
    if !glob_pattern.is_empty() {
        rg_args.push("--glob".to_string());
        rg_args.push(glob_pattern.to_string());
    }
    rg_args.push(old_pattern.to_string());
    rg_args.push(".".to_string());

    let output = Command::new("rg")
        .args(&rg_args)
        .current_dir(&workspace_path)
        .output()
        .context("Failed to run rg. Is ripgrep installed?")?;

    if !output.status.success() && output.stdout.is_empty() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            bail!("rg error: {}", stderr.trim());
        }
        bail!("No files contain the pattern '{}'", old_pattern);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.is_empty() {
        bail!("No files contain the pattern '{}'", old_pattern);
    }

    let files: Vec<PathBuf> = stdout
        .lines()
        .filter(|l| !l.is_empty())
        .map(|l| PathBuf::from(l.trim_start_matches("./")))
        .collect();

    let mut changes: Vec<SedChange> = Vec::new();
    let mut originals: HashMap<PathBuf, String> = HashMap::new();

    for file in &files {
        let full_path = workspace_path.join(file);
        let content = std::fs::read_to_string(&full_path)
            .with_context(|| format!("Failed to read {}", file.display()))?;

        originals.insert(file.clone(), content.clone());

        for (byte_start, byte_end) in find_all_occurrences(&content, old_pattern) {
            let (line, column) = line_col_for_offset(&content, byte_start);
            let word_boundary = is_word_boundary_match(&content, byte_start, byte_end);

            // With -w flag, skip non-word-boundary matches entirely
            if word_only && !word_boundary {
                continue;
            }

            let context_line = extract_context_line(&content, line);

            changes.push(SedChange {
                file: file.clone(),
                byte_start,
                byte_end,
                line,
                column,
                old_text: old_pattern.to_string(),
                new_text: new_pattern.to_string(),
                confirmed: true,
                context_line,
                word_boundary,
            });
        }
    }

    if changes.is_empty() {
        if word_only {
            bail!("No word-boundary occurrences of '{}' found", old_pattern);
        }
        bail!("No occurrences found");
    }

    app.sed_changes = changes;
    app.sed_originals = originals;

    let (buffer_content, header_lines) = build_buffer_content(app);
    let syntax_ref = ensure_diff_syntax(app)?;

    let existing_id = find_sed_buffer_id(app);
    if let Some(id) = existing_id {
        while app.workspace.current_buffer.as_ref().and_then(|b| b.id) != Some(id) {
            app.workspace.next_buffer();
        }
        if let Some(buf) = app.workspace.current_buffer.as_mut() {
            buf.replace(buffer_content);
            buf.cursor.move_to(Position {
                line: header_lines,
                offset: 0,
            });
            if buf.syntax_definition.is_none() {
                buf.syntax_definition = Some(syntax_ref);
            }
        }
    } else {
        let mut sed_buffer = scribe::Buffer::new();
        sed_buffer.path = Some(PathBuf::from(SED_BUFFER_PATH));
        sed_buffer.insert(buffer_content);
        sed_buffer.cursor.move_to(Position {
            line: header_lines,
            offset: 0,
        });
        sed_buffer.syntax_definition = Some(syntax_ref);
        util::add_buffer(sed_buffer, app)?;
        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            app.view.buffer_registry.register(
                buf.id,
                BufferMetadata {
                    buffer_type: BufferType::SedDiff,
                },
            );
        }
    }

    commands::application::switch_to_normal_mode(app)?;
    Ok(())
}

fn parse_change_index(line: &str) -> Option<usize> {
    let hash_pos = line.find('#')?;
    let after_hash = &line[hash_pos + 1..];
    let end = after_hash
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_hash.len());
    after_hash[..end].parse().ok()
}

pub fn toggle(app: &mut Application) -> Result {
    let buf = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    let is_sed = buf
        .path
        .as_ref()
        .map(|p| p.to_string_lossy() == SED_BUFFER_PATH)
        .unwrap_or(false);
    if !is_sed {
        bail!("Not a sed diff buffer");
    }

    let cursor_line = buf.cursor.line;
    let data = buf.data();
    let line_content = data.lines().nth(cursor_line).unwrap_or("");

    if let Some(idx) = parse_change_index(line_content) {
        if idx < app.sed_changes.len() {
            app.sed_changes[idx].confirmed = !app.sed_changes[idx].confirmed;

            let cursor_pos: Position = *app.workspace.current_buffer.as_ref().unwrap().cursor;
            let (new_content, _) = build_buffer_content(app);
            app.workspace
                .current_buffer
                .as_mut()
                .unwrap()
                .replace(new_content);
            app.workspace
                .current_buffer
                .as_mut()
                .unwrap()
                .cursor
                .move_to(cursor_pos);

            move_to_next_marker(app);
        }
    }

    Ok(())
}

fn move_to_next_marker(app: &mut Application) {
    let buf = app.workspace.current_buffer.as_ref().unwrap();
    let data = buf.data();
    let current_line = buf.cursor.line;
    let total_lines = data.lines().count();

    for i in 1..=total_lines {
        let check_line = (current_line + i) % total_lines;
        if let Some(line_text) = data.lines().nth(check_line) {
            if line_text.starts_with("[ ]") || line_text.starts_with("[y]") {
                app.workspace
                    .current_buffer
                    .as_mut()
                    .unwrap()
                    .cursor
                    .move_to(Position {
                        line: check_line,
                        offset: 0,
                    });
                return;
            }
        }
    }
}

pub fn toggle_all(app: &mut Application) -> Result {
    let buf = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    let is_sed = buf
        .path
        .as_ref()
        .map(|p| p.to_string_lossy() == SED_BUFFER_PATH)
        .unwrap_or(false);
    if !is_sed {
        bail!("Not a sed diff buffer");
    }

    let any_excluded = app.sed_changes.iter().any(|c| !c.confirmed);
    for change in &mut app.sed_changes {
        change.confirmed = any_excluded;
    }

    let cursor_pos: Position = *app.workspace.current_buffer.as_ref().unwrap().cursor;
    let (new_content, _) = build_buffer_content(app);
    app.workspace
        .current_buffer
        .as_mut()
        .unwrap()
        .replace(new_content);
    app.workspace
        .current_buffer
        .as_mut()
        .unwrap()
        .cursor
        .move_to(cursor_pos);

    Ok(())
}

pub fn apply(app: &mut Application) -> Result {
    let buf = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    let is_sed = buf
        .path
        .as_ref()
        .map(|p| p.to_string_lossy() == SED_BUFFER_PATH)
        .unwrap_or(false);
    if !is_sed {
        bail!("Not a sed diff buffer");
    }

    let mut file_changes: HashMap<PathBuf, Vec<usize>> = HashMap::new();
    for (idx, change) in app.sed_changes.iter().enumerate() {
        if change.confirmed {
            file_changes
                .entry(change.file.clone())
                .or_default()
                .push(idx);
        }
    }

    if file_changes.is_empty() {
        bail!("All changes excluded. Press 'n' to toggle back, or 'q' to cancel.");
    }

    let workspace_path = app.workspace.path.clone();
    let mut affected_files: Vec<PathBuf> = Vec::new();

    for (file, change_indices) in &file_changes {
        let original = app
            .sed_originals
            .get(file)
            .with_context(|| format!("No original content for {}", file.display()))?;

        let mut sorted = change_indices.clone();
        sorted.sort_by(|a, b| {
            app.sed_changes[*b]
                .byte_start
                .cmp(&app.sed_changes[*a].byte_start)
        });

        let mut content = original.clone();
        for &idx in &sorted {
            let change = &app.sed_changes[idx];

            if content.len() < change.byte_end
                || content[change.byte_start..change.byte_end] != change.old_text
            {
                bail!(
                    "Content mismatch at {} byte {} - file may have changed since :sed was run. Re-run :sed.",
                    file.display(),
                    change.byte_start
                );
            }

            content.replace_range(change.byte_start..change.byte_end, &change.new_text);
        }

        let full_path = workspace_path.join(file);
        std::fs::write(&full_path, &content)
            .with_context(|| format!("Failed to write {}", file.display()))?;

        affected_files.push(file.clone());
    }

    let confirmed_count = file_changes.values().map(|v| v.len()).sum::<usize>();
    app.sed_changes.clear();
    app.sed_originals.clear();

    let id = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?
        .id;
    {
        let buf_ref = app
            .workspace
            .current_buffer
            .as_ref()
            .context(BUFFER_MISSING)?;
        app.view.forget_buffer(buf_ref)?;
    }
    app.workspace.close_current_buffer();
    app.view.buffer_registry.unregister(id);

    reload_affected_buffers(app, &affected_files, &workspace_path)?;

    app.popup = Some((
        "Sed Applied".to_string(),
        vec![format!(
            "{} change(s) across {} file(s)",
            confirmed_count,
            affected_files.len()
        )],
    ));

    Ok(())
}

fn reload_affected_buffers(
    app: &mut Application,
    affected_files: &[PathBuf],
    workspace_path: &std::path::Path,
) -> Result {
    let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
    let mut first = true;
    loop {
        if !first && app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
            break;
        }
        first = false;
        if let Some(open_buf) = app.workspace.current_buffer.as_mut() {
            if let Some(ref path) = open_buf.path {
                let rel = path.strip_prefix(workspace_path).unwrap_or(path);
                if affected_files.iter().any(|f| f.as_path() == rel) {
                    let _ = open_buf.reload();
                }
            }
        }
        app.workspace.next_buffer();
    }
    Ok(())
}

fn find_sed_buffer_id(app: &mut Application) -> Option<usize> {
    let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
    let mut first = true;
    loop {
        if !first && app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
            break;
        }
        first = false;
        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            let is_sed = buf
                .path
                .as_ref()
                .map(|p| p.to_string_lossy() == SED_BUFFER_PATH)
                .unwrap_or(false);
            if is_sed {
                return buf.id;
            }
        }
        app.workspace.next_buffer();
    }
    None
}
