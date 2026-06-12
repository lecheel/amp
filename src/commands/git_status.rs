use crate::commands::{self, Result};
use crate::errors::*;
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
    - match: '^(On branch|Staged|Unstaged|Untracked|Branches|Nothing to commit)'
      scope: keyword.other.git-status
    - match: '^\* '
      scope: constant.numeric.git-status
    - match: '^[MADRCU] '
      scope: markup.inserted.git-status
    - match: '^ [MADRCU] '
      scope: markup.changed.git-status
    - match: '^\?\?'
      scope: markup.deleted.git-status
    - match: 'stash@\{\d+\}'
      scope: constant.numeric.git-status
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

    // Get current branch
    let branch = repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()))
        .unwrap_or_else(|| "unknown".to_string());

    // Run git status --short
    let status_output = Command::new("git")
        .args(["status", "--short"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git status")?;
    let status_stdout = String::from_utf8_lossy(&status_output.stdout);

    // Get top 5 branches sorted by most recent committer date
    let branch_output = Command::new("git")
        .args(["branch", "--sort=-committerdate"])
        .current_dir(repo_path)
        .output()
        .context("Failed to list branches")?;
    let branch_stdout = String::from_utf8_lossy(&branch_output.stdout);

    // Build buffer content
    let mut content = format!("On branch {}\n\n", branch);

    let mut staged_lines = Vec::new();
    let mut unstaged_lines = Vec::new();
    let mut untracked_lines = Vec::new();

    for line in status_stdout.lines() {
        if line.len() < 4 {
            continue;
        }
        let index_status = line.chars().next();
        let worktree_status = line.chars().nth(1);

        if index_status == Some('?') && worktree_status == Some('?') {
            untracked_lines.push(line.to_string());
        } else {
            if index_status.map_or(false, |c| c != ' ' && c != '?') {
                staged_lines.push(line.to_string());
            }
            if worktree_status.map_or(false, |c| c != ' ' && c != '?') {
                unstaged_lines.push(line.to_string());
            }
        }
    }

    if !staged_lines.is_empty() {
        content.push_str("Staged:\n");
        for line in &staged_lines {
            content.push_str(line);
            content.push('\n');
        }
        content.push('\n');
    }

    if !unstaged_lines.is_empty() {
        content.push_str("Unstaged:\n");
        for line in &unstaged_lines {
            content.push_str(line);
            content.push('\n');
        }
        content.push('\n');
    }

    if !untracked_lines.is_empty() {
        content.push_str("Untracked:\n");
        for line in &untracked_lines {
            content.push_str(line);
            content.push('\n');
        }
        content.push('\n');
    }

    if staged_lines.is_empty() && unstaged_lines.is_empty() && untracked_lines.is_empty() {
        content.push_str("Nothing to commit, working tree clean\n\n");
    }

    content.push_str("Branches (top 5):\n");
    for line in branch_stdout.lines().take(5) {
        content.push_str(line);
        content.push('\n');
    }

    content.push('\n');
    content.push_str("s: stage/unstage   z: stash   q: close   r: refresh   enter: open file\n");

    // Find or create the git status buffer
    let existing_id = find_git_status_buffer_id(app);
    if let Some(id) = existing_id {
        while app.workspace.current_buffer.as_ref().and_then(|b| b.id) != Some(id) {
            app.workspace.next_buffer();
        }
        let syntax_ref = ensure_git_status_syntax(app)?;
        if let Some(buf) = app.workspace.current_buffer.as_mut() {
            buf.replace(content);
            buf.cursor.move_to(Position { line: 0, offset: 0 });
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

/// Extract the file path from a git status --short line.
/// Format: `XY PATH` where X=index status, Y=worktree status.
/// For renames: `XY OLD_PATH -> NEW_PATH`, returns NEW_PATH.
fn extract_file_path(line: &str) -> Option<&str> {
    if line.len() < 4 {
        return None;
    }
    let status = &line[..2];
    let rest = &line[3..];

    let first = status.chars().next()?;
    let second = status.chars().nth(1)?;

    // Two spaces means it's not a valid status line (e.g. branch name)
    if first == ' ' && second == ' ' {
        return None;
    }

    if !first.is_ascii_uppercase() && first != ' ' && first != '?' {
        return None;
    }
    if !second.is_ascii_uppercase() && second != ' ' && second != '?' {
        return None;
    }

    // For renamed files, take the new path (after " -> ")
    if let Some(arrow_pos) = rest.find(" -> ") {
        Some(&rest[arrow_pos + 4..])
    } else {
        Some(rest)
    }
}

/// Determine which section the cursor is in by scanning backwards for a section header.
fn current_section(data: &str, line_idx: usize) -> Option<&str> {
    for i in (0..=line_idx).rev() {
        if let Some(line) = data.lines().nth(i) {
            let trimmed = line.trim();
            if trimmed.starts_with("Staged:") {
                return Some("staged");
            } else if trimmed.starts_with("Unstaged:") {
                return Some("unstaged");
            } else if trimmed.starts_with("Untracked:") {
                return Some("untracked");
            } else if trimmed.starts_with("Branches") {
                return Some("branches");
            }
        }
    }
    None
}

/// Toggle staging of the file under the cursor.
/// In the "Staged" section: unstages (git reset HEAD -- <path>).
/// In "Unstaged"/"Untracked" sections: stages (git add <path>).
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

    // Save file path for cursor restoration after refresh
    let target_path = file_path.to_string();

    // Refresh the git status buffer
    show(app)?;

    // Try to position cursor on the same file in the refreshed content
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

/// Open the file under the cursor in the git status buffer.
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

    let file_path = extract_file_path(current_line).context("No file path on this line")?;

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

/// Show stash entries as a popup overlay.
pub fn show_stash(app: &mut Application) -> Result {
    let repo = app.repository.as_ref().context("No repository available")?;
    let repo_path = repo.workdir().context("No working directory")?;

    let output = Command::new("git")
        .args(["stash", "list"])
        .current_dir(repo_path)
        .output()
        .context("Failed to run git stash list")?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !stderr.is_empty() && stdout.is_empty() {
        bail!("git stash list error: {}", stderr.trim());
    }

    let stash_lines: Vec<String> = if stdout.is_empty() {
        vec!["No stash entries found.".to_string()]
    } else {
        stdout.lines().take(10).map(|l| l.to_string()).collect()
    };

    app.popup = Some(("Git Stash".to_string(), stash_lines));
    Ok(())
}
