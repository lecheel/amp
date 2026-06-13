use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::Mode;
use crate::models::application::{Application, BufferMetadata, BufferType};
use crate::util;
use scribe::buffer::Position;
use std::path::PathBuf;
use std::process::Command;
use syntect::parsing::{SyntaxDefinition, SyntaxSet};

const GIT_STATUS_BUFFER_PATH: &str = "[Git Status]";

const GIT_STATUS_SYNTAX_YAML: &str = r#"
%YAML 1.2
---
name: Git Status
scope: text.git-status
contexts:
  main:
    - match: '^(  Stage Changes|  Unstage Changes|  Untracked Files)'
      scope: keyword.other.git-status
    - match: '^(------ (Branch|Stash) ------|Nothing to commit)'
      scope: keyword.other.git-status
    - match: '^  ─+$'
      scope: comment.line.git-status
    - match: '^\s+[MADRCU]\s\s'
      scope: markup.inserted.git-status
    - match: '^\s+\s[MADRCU]\s'
      scope: markup.changed.git-status
    - match: 'stash@\{\d+\}'
      scope: constant.numeric.git-status
    - match: '^\s+\[s\].*$'
      scope: comment.line.git-status
"#;

fn ensure_git_status_syntax(
    app: &mut Application,
) -> anyhow::Result<syntect::parsing::SyntaxReference> {
    if let Some(syn) = app
        .workspace
        .syntax_set
        .find_syntax_by_name("Git Status")
        .cloned()
    {
        return Ok(syn);
    }
    let syntax_def = SyntaxDefinition::load_from_str(GIT_STATUS_SYNTAX_YAML, true, None)
        .context("Failed to parse Git Status syntax definition")?;
    let mut builder =
        std::mem::replace(&mut app.workspace.syntax_set, SyntaxSet::new()).into_builder();
    builder.add(syntax_def);
    app.workspace.syntax_set = builder.build();
    app.workspace
        .syntax_set
        .find_syntax_by_name("Git Status")
        .cloned()
        .context("Failed to find just-added Git Status syntax")
}

pub fn show(app: &mut Application) -> Result {
    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;

    let branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    let status_output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git status")?;
    let status_stdout = String::from_utf8_lossy(&status_output.stdout);

    let branch_output = Command::new("git")
        .args([
            "branch",
            "--sort=-committerdate",
            "--format=%(HEAD) %(refname:short) %(committerdate:relative)",
        ])
        .current_dir(repo_path)
        .output()
        .context("Failed to list branches")?;
    let branch_stdout = String::from_utf8_lossy(&branch_output.stdout);

    let stash_output = Command::new("git")
        .args(["stash", "list"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git stash list")?;
    let stash_stdout = String::from_utf8_lossy(&stash_output.stdout);

    let separator = "  ────────────────────────────────────────";

    let mut content = String::new();

    let mut staged_lines = Vec::new();
    let mut unstaged_lines = Vec::new();
    let mut untracked_paths = Vec::new();

    for line in status_stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let index_status = line.chars().next();
        let worktree_status = line.chars().nth(1);
        if index_status == Some('?') && worktree_status == Some('?') {
            let path = extract_file_path_from_short(line).unwrap_or("");
            if !path.is_empty() {
                untracked_paths.push(path.to_string());
            }
        } else {
            if index_status.map_or(false, |c| c != ' ' && c != '?') {
                staged_lines.push(line.to_string());
            }
            if worktree_status.map_or(false, |c| c != ' ' && c != '?') {
                unstaged_lines.push(line.to_string());
            }
        }
    }

    // Stage Changes
    content.push_str(&format!("  Stage Changes ({})\n", staged_lines.len()));
    content.push_str(separator);
    content.push('\n');
    if staged_lines.is_empty() {
        content.push_str("    (none)\n");
    } else {
        for line in &staged_lines {
            content.push_str("    ");
            content.push_str(line);
            content.push('\n');
        }
    }
    content.push('\n');

    // Unstage Changes
    content.push_str(&format!("  Unstage Changes ({})\n", unstaged_lines.len()));
    content.push_str(separator);
    content.push('\n');
    if unstaged_lines.is_empty() {
        content.push_str("    (none)\n");
    } else {
        for line in &unstaged_lines {
            content.push_str("    ");
            content.push_str(line);
            content.push('\n');
        }
    }
    content.push('\n');

    // Untracked Files
    content.push_str(&format!("  Untracked Files ({})\n", untracked_paths.len()));
    content.push_str(separator);
    content.push('\n');
    if untracked_paths.is_empty() {
        content.push_str("    (none)\n");
    } else {
        for path in &untracked_paths {
            content.push_str("    ");
            content.push_str(path);
            content.push('\n');
        }
    }
    content.push('\n');

    if staged_lines.is_empty() && unstaged_lines.is_empty() && untracked_paths.is_empty() {
        content.push_str("  Nothing to commit, working tree clean\n\n");
    }

    // Branches (top 5)
    content.push_str("  ------ Branch ------\n");
    content.push_str(separator);
    content.push('\n');
    for line in branch_stdout.lines().take(5) {
        let trimmed = line.trim();
        if trimmed.starts_with('*') {
            content.push_str("    ");
        } else {
            content.push_str("      ");
        }
        content.push_str(trimmed);
        content.push('\n');
    }
    content.push('\n');

    // Stash
    let stash_entries: Vec<&str> = stash_stdout.lines().take(5).collect();
    content.push_str("  ------ Stash ------\n");
    content.push_str(separator);
    content.push('\n');
    if stash_entries.is_empty() {
        content.push_str("    (none)\n");
    } else {
        for entry in &stash_entries {
            content.push_str("    ");
            content.push_str(entry);
            content.push('\n');
        }
    }
    content.push('\n');

    // Footer — show confirmation prompt if pending, otherwise normal footer
    content.push_str(separator);
    content.push('\n');
    if let Some(ref stash_ref) = app.pending_stash_ref {
        content.push_str(&format!(
            "  {} (p)pop (d)drop (a)apply (other)dismiss\n",
            stash_ref
        ));
    } else {
        content.push_str("  [s] Toggle staged  [Enter] Open file  [z] stash  [q] Close\n");
    }

    let existing_id = find_git_status_buffer_id(app);
    if let Some(id) = existing_id {
        while app.workspace.current_buffer.as_ref().and_then(|b| b.id) != Some(id) {
            app.workspace.next_buffer();
        }
        let syntax_ref = ensure_git_status_syntax(app)?;
        if let Some(buf) = app.workspace.current_buffer.as_mut() {
            buf.replace(content);
            if buf.syntax_definition.is_none() {
                buf.syntax_definition = Some(syntax_ref);
            }
        }
    } else {
        let mut gs_buffer = scribe::Buffer::new();
        gs_buffer.path = Some(PathBuf::from(GIT_STATUS_BUFFER_PATH));
        gs_buffer.insert(content);
        gs_buffer.cursor.move_to(Position { line: 0, offset: 0 });
        gs_buffer.syntax_definition = Some(ensure_git_status_syntax(app)?);
        util::add_buffer(gs_buffer, app)?;
        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            app.view.buffer_registry.register(
                buf.id,
                BufferMetadata {
                    buffer_type: BufferType::GitStatus,
                },
            );
        }
    }

    commands::application::switch_to_normal_mode(app)?;
    Ok(())
}

fn extract_file_path_from_short(line: &str) -> Option<&str> {
    if line.len() < 4 {
        return None;
    }
    let rest = &line[3..];
    if let Some(arrow_pos) = rest.find(" -> ") {
        Some(&rest[arrow_pos + 4..])
    } else {
        Some(rest)
    }
}

fn find_git_status_buffer_id(app: &mut Application) -> Option<usize> {
    let start_id = app.workspace.current_buffer.as_ref().map(|b| b.id);
    let mut first = true;
    loop {
        if !first && app.workspace.current_buffer.as_ref().map(|b| b.id) == start_id {
            break;
        }
        first = false;
        if let Some(buf) = app.workspace.current_buffer.as_ref() {
            let is_gs = buf
                .path
                .as_ref()
                .map(|p| p.to_string_lossy() == GIT_STATUS_BUFFER_PATH)
                .unwrap_or(false);
            if is_gs {
                return buf.id;
            }
        }
        app.workspace.next_buffer();
    }
    None
}

fn extract_file_path(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed == "(none)" {
        return None;
    }

    // Skip non-file lines
    if trimmed.starts_with("Stage Changes")
        || trimmed.starts_with("Unstage Changes")
        || trimmed.starts_with("Untracked Files")
        || trimmed.starts_with("──")
        || trimmed.starts_with("------")
        || trimmed.starts_with('[')
        || trimmed.contains("Nothing to commit")
        || trimmed.starts_with("stash@{")
        || trimmed.contains("Pop stash@{")
        || trimmed.starts_with('*')
    {
        return None;
    }

    // XY format: first two chars are git status codes (M, A, D, R, C, U, ?, space)
    // Skip both, then trim any leading spaces before the filename
    if trimmed.len() >= 3 {
        let bytes = trimmed.as_bytes();
        let first = bytes[0] as char;
        let second = bytes[1] as char;

        let first_is_status = first.is_ascii_uppercase() || first == '?' || first == ' ';
        let second_is_status = second.is_ascii_uppercase() || second == '?' || second == ' ';
        let not_both_spaces = !(first == ' ' && second == ' ');

        if first_is_status && second_is_status && not_both_spaces {
            // Skip the 2-char status prefix, then trim all leading spaces
            let rest = trimmed[2..].trim_start();
            if rest.is_empty() {
                return None;
            }
            // Handle renamed files: "old -> new"
            if let Some(arrow_pos) = rest.find(" -> ") {
                return Some(&rest[arrow_pos + 4..]);
            } else {
                return Some(rest);
            }
        }
    }

    // Plain filename (untracked section without ?? prefix)
    Some(trimmed)
}

fn extract_stash_ref(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("stash@{") {
        return None;
    }
    let end = trimmed.find(':').unwrap_or(trimmed.len());
    Some(trimmed[..end].to_string())
}

fn current_section(data: &str, line_idx: usize) -> Option<&str> {
    for i in (0..=line_idx).rev() {
        if let Some(line) = data.lines().nth(i) {
            if line.contains("Stage Changes") {
                return Some("staged");
            } else if line.contains("Unstage Changes") {
                return Some("unstaged");
            } else if line.contains("Untracked Files") {
                return Some("untracked");
            } else if line.contains("Stash") {
                return Some("stash");
            } else if line.contains("Branch") {
                return Some("info");
            }
        }
    }
    None
}

pub fn stage_file(app: &mut Application) -> Result {
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
    let file_path = extract_file_path(current_line).context("No file path on this line")?;
    let section = current_section(&data, current_line_idx);

    if section == Some("info") || section == Some("stash") {
        bail!("No file on this line");
    }

    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;

    let result = match section {
        Some("staged") => Command::new("git")
            .args(["reset", "HEAD", "--", file_path])
            .current_dir(repo_path)
            .output()
            .context("Failed to run git reset")?,
        _ => Command::new("git")
            .args(["add", file_path])
            .current_dir(repo_path)
            .output()
            .context("Failed to run git add")?,
    };

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("git command failed: {}", stderr.trim());
    }

    let target_path = file_path.to_string();
    show(app)?;
    if let Some(buf) = app.workspace.current_buffer.as_mut() {
        let data = buf.data();
        for (i, line) in data.lines().enumerate() {
            if extract_file_path(line).map_or(false, |p| p == target_path) {
                buf.cursor.move_to(Position { line: i, offset: 0 });
                break;
            }
        }
    }
    Ok(())
}

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

    // Check if this is a stash line — enter confirmation mode
    if let Some(stash_ref) = extract_stash_ref(current_line) {
        app.pending_stash_ref = Some(stash_ref);
        show(app)?;
        // Move cursor to the footer confirmation line
        if let Some(buf) = app.workspace.current_buffer.as_mut() {
            let last_line = buf.line_count().saturating_sub(1);
            buf.cursor.move_to(Position {
                line: last_line,
                offset: 0,
            });
        }
        return Ok(());
    }

    let file_path = extract_file_path(current_line).context("No file path on this line")?;
    let section = current_section(&data, current_line_idx);
    if section == Some("info") || section == Some("stash") {
        bail!("No file on this line");
    }
    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;
    let absolute_path = repo_path.join(file_path);
    if !absolute_path.exists() {
        bail!("File not found: {}", absolute_path.display());
    }
    crate::util::open_buffer(&absolute_path, app)?;
    commands::view::scroll_to_cursor(app)?;
    Ok(())
}

/// Switch to ex mode with `:stash ` pre-filled so user can type a description.
/// If they press Enter immediately, it auto-stashes with a default message.
pub fn prompt_stash(app: &mut Application) -> Result {
    app.switch_to(ModeKey::Ex);
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.reset();
        mode.input.push_str(":stash ");
    }
    Ok(())
}

/// Drop a stash entry.
pub fn drop_stash(app: &mut Application) -> Result {
    let stash_ref = app
        .pending_stash_ref
        .take()
        .context("No stash reference to drop")?;
    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;

    let result = Command::new("git")
        .args(["stash", "drop", &stash_ref])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git stash drop")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("git stash drop failed: {}", stderr.trim());
    }

    show(app)
}

/// Apply a stash entry without removing it from the stash list.
pub fn apply_stash(app: &mut Application) -> Result {
    let stash_ref = app
        .pending_stash_ref
        .take()
        .context("No stash reference to apply")?;
    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;

    let result = Command::new("git")
        .args(["stash", "apply", &stash_ref])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git stash apply")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("git stash apply failed: {}", stderr.trim());
    }

    show(app)
}

/// Actually perform the stash. Called from ex mode's accept_input after parsing `:stash [msg]`.
/// If message is empty, auto-stash with a generated message.
pub fn do_stash(app: &mut Application, message: &str) -> Result {
    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;

    let result = if message.is_empty() {
        Command::new("git")
            .args(["stash", "push"])
            .current_dir(repo_path)
            .output()
            .context("Failed to run git stash")?
    } else {
        Command::new("git")
            .args(["stash", "push", "-m", message])
            .current_dir(repo_path)
            .output()
            .context("Failed to run git stash push -m")?
    };

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("git stash failed: {}", stderr.trim());
    }

    show(app)
}

/// Pop a stash entry. Called when user confirms (y) in the pending_stash_ref state.
pub fn pop_stash(app: &mut Application) -> Result {
    let stash_ref = app
        .pending_stash_ref
        .take()
        .context("No stash reference to pop")?;
    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;

    let result = Command::new("git")
        .args(["stash", "pop", &stash_ref])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git stash pop")?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        bail!("git stash pop failed: {}", stderr.trim());
    }

    show(app)
}

use crate::models::application::modes::ModeKey;
