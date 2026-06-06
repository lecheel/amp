use crate::errors::*;
use git2::Repository;
use scribe::Buffer;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitGutterStatus {
    Unchanged,
    Added,
    Modified,
    Deleted,
}

/// Computes the git gutter status for each line of the buffer
/// by comparing the current buffer content against the git index.
pub fn line_statuses(repo: &Repository, buffer: &Buffer) -> Result<Vec<GitGutterStatus>> {
    let path = buffer.path.as_ref().context(BUFFER_PATH_MISSING)?;
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;
    let relative_path = path
        .strip_prefix(workdir)
        .with_context(|| format!("Could not determine relative path for {:?}", path))?;

    // Get the content from the git index
    let index = repo.index().context("Failed to get git index")?;
    let entry = index.get_path(relative_path, 0);

    let old_content = match entry {
        Some(entry) => {
            let blob = repo
                .find_blob(entry.id)
                .context("Failed to find blob in git repository")?;
            String::from_utf8_lossy(blob.content()).to_string()
        }
        None => String::new(), // Untracked file — all lines are "added"
    };

    let new_content = buffer.data();
    let mut statuses = compute_line_statuses(&old_content, &new_content);

    // Ensure we have enough entries for all rendered lines
    // (including the extra line after a trailing newline)
    while statuses.len() < buffer.line_count() {
        statuses.push(GitGutterStatus::Unchanged);
    }

    Ok(statuses)
}

/// Computes line-by-line diff status between old and new content
/// using a standard LCS-based diff algorithm.
fn compute_line_statuses(old: &str, new: &str) -> Vec<GitGutterStatus> {
    let old_lines: Vec<&str> = if old.is_empty() {
        Vec::new()
    } else {
        old.lines().collect()
    };
    let new_lines: Vec<&str> = if new.is_empty() {
        Vec::new()
    } else {
        new.lines().collect()
    };

    // If the old file was empty, all lines are added
    if old_lines.is_empty() {
        return vec![GitGutterStatus::Added; new_lines.len().max(1)];
    }

    // If the new file is empty, nothing to show
    if new_lines.is_empty() {
        return Vec::new();
    }

    // Compute LCS using dynamic programming
    let m = old_lines.len();
    let n = new_lines.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            if old_lines[i - 1] == new_lines[j - 1] {
                dp[i][j] = dp[i - 1][j - 1] + 1;
            } else {
                dp[i][j] = dp[i - 1][j].max(dp[i][j - 1]);
            }
        }
    }

    // Backtrack to determine diff operations
    enum Op {
        Keep(usize),
        Delete,
        Insert(usize),
    }

    let mut ops = Vec::new();
    let mut i = m;
    let mut j = n;

    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            ops.push(Op::Keep(j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push(Op::Insert(j - 1));
            j -= 1;
        } else {
            ops.push(Op::Delete);
            i -= 1;
        }
    }

    ops.reverse();

    // Convert operations to line statuses by grouping into hunks
    let mut statuses = vec![GitGutterStatus::Unchanged; n];
    let mut idx = 0;

    while idx < ops.len() {
        match ops[idx] {
            Op::Keep(_) => {
                idx += 1;
            }
            Op::Delete => {
                // Count consecutive deletes
                // let mut delete_count = 0;
                while idx < ops.len() && matches!(ops[idx], Op::Delete) {
                    // delete_count += 1;
                    idx += 1;
                }

                // Collect following inserts (these represent modifications)
                let mut insert_indices = Vec::new();
                while idx < ops.len() {
                    if let Op::Insert(new_idx) = ops[idx] {
                        insert_indices.push(new_idx);
                        idx += 1;
                    } else {
                        break;
                    }
                }

                if !insert_indices.is_empty() {
                    // Lines are modified (old lines replaced by new lines)
                    for &new_idx in &insert_indices {
                        statuses[new_idx] = GitGutterStatus::Modified;
                    }
                } else {
                    // Pure deletion — mark the next unchanged line
                    if idx < ops.len() {
                        if let Op::Keep(new_idx) = ops[idx] {
                            if new_idx < statuses.len() {
                                statuses[new_idx] = GitGutterStatus::Deleted;
                            }
                        }
                    }
                }
            }
            Op::Insert(new_idx) => {
                // Pure insertion (no preceding deletion in this hunk)
                statuses[new_idx] = GitGutterStatus::Added;
                idx += 1;
            }
        }
    }

    statuses
}

#[cfg(test)]
mod tests {
    use super::{compute_line_statuses, GitGutterStatus};

    #[test]
    fn empty_old_file_marks_all_lines_as_added() {
        let statuses = compute_line_statuses("", "line1\nline2\n");
        assert_eq!(
            statuses,
            vec![GitGutterStatus::Added, GitGutterStatus::Added]
        );
    }

    #[test]
    fn identical_content_marks_all_lines_as_unchanged() {
        let content = "line1\nline2\nline3\n";
        let statuses = compute_line_statuses(content, content);
        assert_eq!(
            statuses,
            vec![
                GitGutterStatus::Unchanged,
                GitGutterStatus::Unchanged,
                GitGutterStatus::Unchanged,
            ]
        );
    }

    #[test]
    fn added_lines_are_detected() {
        let old = "line1\nline3\n";
        let new = "line1\nline2\nline3\n";
        let statuses = compute_line_statuses(old, new);
        assert_eq!(
            statuses,
            vec![
                GitGutterStatus::Unchanged,
                GitGutterStatus::Added,
                GitGutterStatus::Unchanged,
            ]
        );
    }

    #[test]
    fn deleted_lines_are_marked_on_adjacent_line() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nline3\n";
        let statuses = compute_line_statuses(old, new);
        assert_eq!(
            statuses,
            vec![GitGutterStatus::Unchanged, GitGutterStatus::Deleted,]
        );
    }

    #[test]
    fn modified_lines_are_detected() {
        let old = "line1\nold_line\nline3\n";
        let new = "line1\nnew_line\nline3\n";
        let statuses = compute_line_statuses(old, new);
        assert_eq!(
            statuses,
            vec![
                GitGutterStatus::Unchanged,
                GitGutterStatus::Modified,
                GitGutterStatus::Unchanged,
            ]
        );
    }

    #[test]
    fn multiple_modifications_in_sequence() {
        let old = "a\nb\nc\n";
        let new = "x\ny\nz\n";
        let statuses = compute_line_statuses(old, new);
        assert_eq!(
            statuses,
            vec![
                GitGutterStatus::Modified,
                GitGutterStatus::Modified,
                GitGutterStatus::Modified,
            ]
        );
    }

    #[test]
    fn mixed_changes_are_detected() {
        let old = "line1\nline2\nline3\nline5\n";
        let new = "line1\nline2\nline4\nline5\nline6\n";
        let statuses = compute_line_statuses(old, new);
        assert_eq!(
            statuses,
            vec![
                GitGutterStatus::Unchanged,
                GitGutterStatus::Unchanged,
                GitGutterStatus::Modified, // line3 -> line4
                GitGutterStatus::Unchanged,
                GitGutterStatus::Added, // line6
            ]
        );
    }
}
