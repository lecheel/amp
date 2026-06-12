pub mod error;
pub mod modes;

use crate::models::application::GitGutterStatus;
use crate::view::{Alignment, Colors, StatusLineData, Style, View};
use git2::{self, Repository, Status};
use scribe::Workspace;
use std::path::Path;

// ── Public unified helper ──────────────────────────────────────

/// Build the standard 4-entry status line used by most modes:
/// `[mode label][⎇ branch] [filename] ──── [modified]`
///
/// Call this BEFORE `view.build_presenter()` to avoid borrow conflicts.
pub fn standard_status_line(
    mode_label: &str,
    mode_colors: Colors,
    workspace: &mut Workspace,
    view: &View,
    repo: &Option<Repository>,
) -> Vec<StatusLineData> {
    let branch = git_branch_line_data(repo);
    let filename = filename_line_data(workspace);
    let modified = modified_status_line_data(workspace, view, repo);
    vec![
        StatusLineData {
            content: format!(" {} ", mode_label),
            style: Style::Default,
            colors: mode_colors,
            alignment: Alignment::Left,
        },
        branch,
        filename,
        modified,
    ]
}

// ── Private helpers ────────────────────────────────────────────

fn path_as_title(path: &Path) -> String {
    format!(" {}", path.to_string_lossy())
}

fn git_branch_line_data(repo: &Option<Repository>) -> StatusLineData {
    let content = if let Some(ref repo) = *repo {
        repo.head()
            .ok()
            .and_then(|head| head.shorthand().map(|s| format!(" ⎇  {} ", s)))
            .unwrap_or_default()
    } else {
        String::new()
    };
    StatusLineData {
        content,
        style: Style::Default,
        colors: Colors::Focused,
        alignment: Alignment::Left,
    }
}

fn filename_line_data(workspace: &mut Workspace) -> StatusLineData {
    let content = workspace
        .current_buffer_path()
        .map(|path| format!(" {} ", path_as_title(path)))
        .unwrap_or_default();
    StatusLineData {
        content,
        style: Style::Default,
        colors: Colors::Focused,
        alignment: Alignment::Expand,
    }
}

fn modified_status_line_data(
    workspace: &mut Workspace,
    view: &View,
    repo: &Option<Repository>,
) -> StatusLineData {
    let buf = workspace.current_buffer.as_ref();
    let modified = buf.map(|b| view.effective_modified(b)).unwrap_or(false);

    // Check file-level git status FIRST
    let file_git_status = buf.and_then(|b| {
        let path = b.path.as_ref()?;
        let repo_ref = repo.as_ref()?;
        let repo_path = repo_ref.workdir()?;
        let relative = path.strip_prefix(repo_path).ok()?;
        let status = repo_ref.status_file(relative).ok()?;
        Some(status)
    });

    // For untracked files, skip line-level diff (it would show every line as added)
    let is_untracked = file_git_status
        .map(|s| s.contains(git2::Status::WT_NEW))
        .unwrap_or(false);

    let content = if is_untracked {
        if modified {
            " [new*] ".to_string()
        } else {
            " [new] ".to_string()
        }
    } else {
        // Count gutter statuses for line-level diff summary
        let (added, modified_lines, deleted) = view
            .gutter_statuses
            .as_ref()
            .map(|statuses| {
                let a = statuses
                    .iter()
                    .filter(|s| **s == GitGutterStatus::Added)
                    .count();
                let m = statuses
                    .iter()
                    .filter(|s| **s == GitGutterStatus::Modified)
                    .count();
                let d = statuses
                    .iter()
                    .filter(|s| **s == GitGutterStatus::Deleted)
                    .count();
                (a, m, d)
            })
            .unwrap_or((0, 0, 0));

        let has_git_changes = added > 0 || modified_lines > 0 || deleted > 0;

        if has_git_changes {
            let mut parts = Vec::new();
            if added > 0 {
                parts.push(format!("+{}", added));
            }
            if modified_lines > 0 {
                parts.push(format!("~{}", modified_lines));
            }
            if deleted > 0 {
                parts.push(format!("-{}", deleted));
            }
            if modified {
                format!(" [{}*] ", parts.join(" "))
            } else {
                format!(" [{}] ", parts.join(" "))
            }
        } else if modified {
            " [+mod] ".to_string()
        } else {
            // Check file-level git status for staged
            let git_indicator: &str = file_git_status
                .and_then(|status| Some(presentable_status_short(&status)))
                .unwrap_or("");

            match git_indicator {
                "" => String::new(),
                s => format!(" [{}] ", s),
            }
        }
    };

    let colors = if modified
        || (!is_untracked && content.contains('+')
            || content.contains('~')
            || content.contains('-'))
    {
        Colors::Warning
    } else {
        Colors::Focused
    };
    StatusLineData {
        content,
        style: Style::Default,
        colors,
        alignment: Alignment::Right,
    }
}

fn presentable_status_short(status: &Status) -> &'static str {
    if status.contains(git2::Status::WT_NEW) {
        if status.contains(git2::Status::INDEX_NEW) {
            "partial"
        } else {
            "new"
        }
    } else if status.contains(git2::Status::INDEX_NEW) {
        "staged"
    } else if status.contains(git2::Status::WT_MODIFIED) {
        if status.contains(git2::Status::INDEX_MODIFIED) {
            "partial"
        } else {
            "mod"
        }
    } else if status.contains(git2::Status::INDEX_MODIFIED) {
        "staged"
    } else {
        "ok"
    }
}

#[cfg(test)]
mod tests {
    use super::presentable_status;
    use git2;

    #[test]
    pub fn presentable_status_returns_untracked_when_status_is_locally_new() {
        let status = git2::Status::WT_NEW;
        assert_eq!(presentable_status(&status), "[untracked]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_ok_when_status_unmodified() {
        let status = git2::Status::CURRENT;
        assert_eq!(presentable_status(&status), "[ok]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_staged_when_only_modified_in_index() {
        let status = git2::Status::INDEX_MODIFIED;
        assert_eq!(presentable_status(&status), "[staged]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_staged_when_new_in_index() {
        let status = git2::Status::INDEX_NEW;
        assert_eq!(presentable_status(&status), "[staged]".to_string());
    }

    #[test]
    pub fn presentable_status_returns_partially_staged_when_modified_locally_and_in_index() {
        let status = git2::Status::WT_MODIFIED | git2::Status::INDEX_MODIFIED;
        assert_eq!(
            presentable_status(&status),
            "[partially staged]".to_string()
        );
    }

    #[test]
    pub fn presentable_status_returns_partially_staged_when_new_locally_and_in_index() {
        let status = git2::Status::WT_NEW | git2::Status::INDEX_NEW;
        assert_eq!(
            presentable_status(&status),
            "[partially staged]".to_string()
        );
    }
}
