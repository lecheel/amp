use crate::commands::{self, Result};
use crate::errors;
use crate::errors::*;
use crate::models::application::git_gutter::{line_statuses, GitGutterStatus};
use crate::models::application::{Application, ClipboardContent, Mode};
use git2;
use regex::Regex;
use std::cmp::Ordering;

pub fn add(app: &mut Application) -> Result {
    let repo = app.repository.as_ref().context("No repository available")?;
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    let mut index = repo.index().context("Couldn't get the repository index")?;
    let buffer_path = buffer.path.as_ref().context(BUFFER_PATH_MISSING)?;
    let repo_path = repo.workdir().context("No path found for the repository")?;
    let relative_path = buffer_path
        .strip_prefix(repo_path)
        .context("Failed to build a relative buffer path")?;

    index
        .add_path(relative_path)
        .context("Failed to add path to index.")?;
    index.write().context("Failed to write index.")
}

pub fn copy_remote_url(app: &mut Application) -> Result {
    if let Some(ref mut repo) = app.repository {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .context(BUFFER_MISSING)?;
        let buffer_path = buffer.path.as_ref().context(BUFFER_PATH_MISSING)?;
        let remote = repo
            .find_remote("origin")
            .context("Couldn't find a remote \"origin\"")?;
        let url = remote.url().context("No URL for remote/origin")?;

        let gh_path = get_gh_path(url)?;

        let repo_path = repo.workdir().context("No path found for the repository")?;
        let relative_path = buffer_path
            .strip_prefix(repo_path)
            .context("Failed to build a relative buffer path")?;

        let status = repo
            .status_file(relative_path)
            .context("Couldn't get status info for the specified path")?;
        if status.contains(git2::Status::WT_NEW) || status.contains(git2::Status::INDEX_NEW) {
            bail!("The provided path doesn't exist in the repository");
        }

        // We want to build URLs that point to an object ID, so that they'll
        // refer to a snapshot of the file as it looks at this very moment.
        let mut revisions = repo
            .revwalk()
            .context("Couldn't build a list of revisions for the repository")?;

        // We need to set a starting point for the commit graph we'll
        // traverse. We want the most recent commit, so start at HEAD.
        revisions
            .push_head()
            .context("Failed to push HEAD to commit graph.")?;

        // Pull the first revision (HEAD).
        let last_oid = revisions
            .next()
            .and_then(|revision| revision.ok())
            .context("Couldn't find a git object ID for this file")?;

        let line_range = match app.mode {
            Mode::SelectLine(ref s) => {
                // Avoid zero-based line numbers.
                let line_1 = buffer.cursor.line + 1;
                let line_2 = s.anchor + 1;

                match line_1.cmp(&line_2) {
                    Ordering::Less => format!("#L{line_1}-L{line_2}"),
                    Ordering::Greater => format!("#L{line_2}-L{line_1}"),
                    Ordering::Equal => format!("#L{line_1}"),
                }
            }
            _ => String::new(),
        };

        let gh_url = format!(
            "https://github.com/{}/blob/{:?}/{}{}",
            gh_path,
            last_oid,
            relative_path.to_string_lossy(),
            line_range
        );

        app.clipboard
            .set_content(ClipboardContent::Inline(gh_url))?;
    } else {
        bail!("No repository available");
    }

    commands::application::switch_to_normal_mode(app)?;

    Ok(())
}

fn get_gh_path(url: &str) -> errors::Result<&str> {
    lazy_static! {
        static ref REGEX: Regex =
            Regex::new(r"^(?:https://|git@)github.com(?::|/)(.*?)(?:.git)?$").unwrap();
    }
    REGEX
        .captures(url)
        .and_then(|c| c.get(1))
        .map(|c| c.as_str())
        .context("Failed to capture remote repo path")
}

/// Reverts the hunk under the cursor back to its state in the git index.
///
/// "Hunk" means the contiguous run of changed lines (Added / Modified / Deleted)
/// that contains the cursor.  The operation is applied entirely to the in-memory
/// buffer; nothing is read from or written to disk.
///
/// Revert rules per line status
/// ─────────────────────────────
///  Added    → delete the line (it didn't exist in the index)
///  Modified → replace the buffer line with the original index line
///  Deleted  → insert the missing index lines before the adjacent line that
///             carries the Deleted marker
pub fn revert_hunk(app: &mut Application) -> Result {
    // ── 1. Gather repo + buffer ──────────────────────────────────────────────
    let repo = app.repository.as_ref().context("No repository available")?;

    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;

    // line_statuses diffs the git-index blob against buffer.data() in memory.
    let statuses = line_statuses(repo, buffer).context("Failed to compute git hunk statuses")?;

    let cursor_line = buffer.cursor.line;

    // ── 2. Find the hunk: contiguous changed lines around the cursor ─────────
    let is_changed = |s: &GitGutterStatus| !matches!(s, GitGutterStatus::Unchanged);

    if statuses.get(cursor_line).map_or(true, |s| !is_changed(s)) {
        bail!("No changed hunk at cursor position");
    }

    // Walk backward
    let hunk_start = {
        let mut s = cursor_line;
        while s > 0 && statuses.get(s - 1).map_or(false, is_changed) {
            s -= 1;
        }
        s
    };

    // Walk forward
    let hunk_end = {
        let mut e = cursor_line;
        while e + 1 < statuses.len() && statuses.get(e + 1).map_or(false, is_changed) {
            e += 1;
        }
        e
    };

    // ── 3. Fetch the index blob content ──────────────────────────────────────
    let path = buffer.path.as_ref().context(BUFFER_PATH_MISSING)?;
    let workdir = repo
        .workdir()
        .context("Repository has no working directory")?;
    let relative_path = path
        .strip_prefix(workdir)
        .context("Failed to build relative buffer path")?;

    let index = repo.index().context("Failed to get git index")?;
    let old_content: String = match index.get_path(relative_path, 0) {
        Some(entry) => {
            let blob = repo
                .find_blob(entry.id)
                .context("Failed to find blob in repository")?;
            String::from_utf8_lossy(blob.content()).into_owned()
        }
        None => String::new(),
    };

    let old_lines: Vec<&str> = if old_content.is_empty() {
        Vec::new()
    } else {
        old_content.lines().collect()
    };

    let new_content = buffer.data();
    let new_lines: Vec<&str> = new_content.lines().collect();

    // ── 4. Build new→old index mapping ───────────────────────────────────────
    //
    // result[new_idx] = Some(old_idx)  for Unchanged AND Modified lines
    //                 = None           for purely Added lines
    //
    // The key fix vs the previous version: we run the same op-sequence walk
    // that compute_line_statuses uses, pairing consecutive Delete+Insert runs
    // so that Modified new lines get Some(old_idx) instead of None.
    let mapping = build_new_to_old_mapping(&old_lines, &new_lines);

    // ── 5. Apply the revert as a single undo group ───────────────────────────
    let buffer = app
        .workspace
        .current_buffer
        .as_mut()
        .context(BUFFER_MISSING)?;

    buffer.start_operation_group();

    // Iterate in REVERSE so deletions/insertions below don't shift line numbers
    // for lines we haven't processed yet.
    let mut new_idx = hunk_end as isize;
    while new_idx >= hunk_start as isize {
        let ni = new_idx as usize;
        match statuses
            .get(ni)
            .copied()
            .unwrap_or(GitGutterStatus::Unchanged)
        {
            GitGutterStatus::Unchanged => {}

            GitGutterStatus::Added => {
                // Line does not exist in the index → delete it.
                delete_buffer_line(buffer, ni);
            }

            GitGutterStatus::Modified => {
                // Replace the buffer line with the original index line.
                // mapping[ni] is now guaranteed to be Some(old_idx) — the bug fix.
                if let Some(old_idx) = mapping.get(ni).copied().flatten() {
                    if let Some(&old_line) = old_lines.get(old_idx) {
                        replace_buffer_line(buffer, ni, old_line);
                    }
                }
            }

            GitGutterStatus::Deleted => {
                // The Deleted marker sits on the next surviving line.
                // Re-insert the missing old lines before it.
                if let Some(missing) = missing_old_lines_before(ni, &mapping, &old_lines) {
                    // Insert in reverse order: each call inserts at the same
                    // position, so the first old line ends up on top.
                    for old_line in missing.iter().rev() {
                        insert_buffer_line_before(buffer, ni, old_line);
                    }
                }
            }
        }

        new_idx -= 1;
    }

    buffer.end_operation_group();

    // Restore cursor (clamped in case lines were deleted).
    let final_line = cursor_line.min(buffer.data().lines().count().saturating_sub(1));
    buffer.cursor.move_to(scribe::buffer::Position {
        line: final_line,
        offset: 0,
    });

    Ok(())
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Diff operation from the LCS backtrack.
enum DiffOp {
    Keep(usize, usize), // (old_idx, new_idx)
    Delete(usize),      // old_idx only — line removed from buffer
    Insert(usize),      // new_idx only — line added to buffer
}

/// Builds the raw LCS-based op sequence for (old_lines, new_lines).
fn build_diff_ops(old_lines: &[&str], new_lines: &[&str]) -> Vec<DiffOp> {
    if old_lines.is_empty() {
        return new_lines
            .iter()
            .enumerate()
            .map(|(j, _)| DiffOp::Insert(j))
            .collect();
    }
    if new_lines.is_empty() {
        return old_lines
            .iter()
            .enumerate()
            .map(|(i, _)| DiffOp::Delete(i))
            .collect();
    }

    let m = old_lines.len();
    let n = new_lines.len();
    let mut dp = vec![vec![0usize; n + 1]; m + 1];

    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if old_lines[i - 1] == new_lines[j - 1] {
                dp[i - 1][j - 1] + 1
            } else {
                dp[i - 1][j].max(dp[i][j - 1])
            };
        }
    }

    let mut ops = Vec::new();
    let (mut i, mut j) = (m, n);
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old_lines[i - 1] == new_lines[j - 1] {
            ops.push(DiffOp::Keep(i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i][j - 1] >= dp[i - 1][j]) {
            ops.push(DiffOp::Insert(j - 1));
            j -= 1;
        } else {
            ops.push(DiffOp::Delete(i - 1));
            i -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Builds `result` where `result[new_idx]`:
///   - `Some(old_idx)` for **Unchanged** lines (LCS match)
///   - `Some(old_idx)` for **Modified** lines (Delete immediately followed by Insert)
///   - `None`          for purely **Added** lines (Insert with no preceding Delete)
///
/// This mirrors the hunk-grouping logic in `git_gutter::compute_line_statuses`
/// so the mapping is always consistent with the displayed gutter markers.
fn build_new_to_old_mapping(old_lines: &[&str], new_lines: &[&str]) -> Vec<Option<usize>> {
    let mut result = vec![None; new_lines.len()];
    if old_lines.is_empty() || new_lines.is_empty() {
        return result;
    }

    let ops = build_diff_ops(old_lines, new_lines);
    let mut idx = 0;

    while idx < ops.len() {
        match ops[idx] {
            DiffOp::Keep(old_i, new_j) => {
                result[new_j] = Some(old_i);
                idx += 1;
            }

            DiffOp::Delete(_) => {
                // Collect the full run of consecutive Deletes.
                let mut deleted_old: Vec<usize> = Vec::new();
                while idx < ops.len() {
                    if let DiffOp::Delete(oi) = ops[idx] {
                        deleted_old.push(oi);
                        idx += 1;
                    } else {
                        break;
                    }
                }

                // Collect the Inserts that immediately follow — these are the
                // Modified new lines (old content replaced by new content).
                let mut inserted_new: Vec<usize> = Vec::new();
                while idx < ops.len() {
                    if let DiffOp::Insert(nj) = ops[idx] {
                        inserted_new.push(nj);
                        idx += 1;
                    } else {
                        break;
                    }
                }

                // Pair them up positionally.
                // - Paired Insert → Modified: gets Some(old_idx) so revert can
                //   look up the original line content.
                // - Unpaired Insert (more inserts than deletes) → pure Added: stays None.
                // - Unpaired Delete (more deletes than inserts) → pure Deleted: no new slot.
                for (k, &new_j) in inserted_new.iter().enumerate() {
                    if let Some(&old_i) = deleted_old.get(k) {
                        result[new_j] = Some(old_i);
                    }
                    // else: stays None (pure Added)
                }
            }

            DiffOp::Insert(new_j) => {
                // Pure insert with no preceding Delete in this run → Added, stays None.
                result[new_j] = None;
                idx += 1;
            }
        }
    }

    result
}

/// Returns the old lines that are "missing" just before new line `new_idx`
/// (i.e. the lines that were deleted and need to be re-inserted for a revert).
///
/// We find the gap between the old_idx of the previous mapped new line and the
/// old_idx of `new_idx` itself.
fn missing_old_lines_before<'a>(
    new_idx: usize,
    mapping: &[Option<usize>],
    old_lines: &[&'a str],
) -> Option<Vec<&'a str>> {
    // The line carrying the Deleted marker is Unchanged, so it has a mapping.
    let old_at = mapping.get(new_idx).copied().flatten()?;

    // Find the old_idx of the closest preceding mapped new line.
    let range_start = (0..new_idx)
        .rev()
        .find_map(|k| mapping.get(k).copied().flatten())
        .map(|v| v + 1) // first old_idx after the previous anchor
        .unwrap_or(0); // deletion is at the very top of the file

    if range_start >= old_at {
        return None; // no gap
    }

    Some(old_lines[range_start..old_at].to_vec())
}

/// Deletes line `line_no` from the buffer, including its trailing newline.
fn delete_buffer_line(buffer: &mut scribe::Buffer, line_no: usize) {
    use scribe::buffer::{Position, Range};

    let data = buffer.data();
    let line_count = data.lines().count();
    if line_no >= line_count {
        return;
    }

    let start = Position {
        line: line_no,
        offset: 0,
    };
    let end = if line_no + 1 < line_count {
        Position {
            line: line_no + 1,
            offset: 0,
        }
    } else {
        Position {
            line: line_no,
            offset: data.lines().nth(line_no).unwrap_or("").chars().count(),
        }
    };

    buffer.delete_range(Range::new(start, end));
}

/// Replaces the text content of line `line_no` with `new_text` (no newline).
fn replace_buffer_line(buffer: &mut scribe::Buffer, line_no: usize, new_text: &str) {
    use scribe::buffer::{Position, Range};

    let current_len = buffer
        .data()
        .lines()
        .nth(line_no)
        .unwrap_or("")
        .chars()
        .count();

    buffer.delete_range(Range::new(
        Position {
            line: line_no,
            offset: 0,
        },
        Position {
            line: line_no,
            offset: current_len,
        },
    ));
    buffer.cursor.move_to(Position {
        line: line_no,
        offset: 0,
    });
    buffer.insert(new_text.to_string());
}

/// Inserts `text` as a new line before `line_no`, pushing existing content down.
fn insert_buffer_line_before(buffer: &mut scribe::Buffer, line_no: usize, text: &str) {
    use scribe::buffer::Position;

    buffer.cursor.move_to(Position {
        line: line_no,
        offset: 0,
    });
    buffer.insert(format!("{text}\n"));
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod revert_hunk_tests {
    use super::{
        build_new_to_old_mapping, delete_buffer_line, insert_buffer_line_before,
        missing_old_lines_before, replace_buffer_line,
    };
    use scribe::Buffer;

    // ── build_new_to_old_mapping ─────────────────────────────────────────────

    #[test]
    fn modified_line_gets_old_idx() {
        // THE BUG: previously returned None for new[1]; now must return Some(1).
        let old = vec!["line1", "old_line", "line3"];
        let new = vec!["line1", "new_line", "line3"];
        let m = build_new_to_old_mapping(&old, &new);
        assert_eq!(
            m[1],
            Some(1),
            "Modified line must map to old_idx so revert can look up old content"
        );
        assert_eq!(&old[m[1].unwrap()], &"old_line");
    }

    #[test]
    fn all_modified_maps_each_line() {
        let old = vec!["a", "b", "c"];
        let new = vec!["x", "y", "z"];
        let m = build_new_to_old_mapping(&old, &new);
        assert_eq!(m, vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn added_line_stays_none() {
        let old = vec!["a", "b"];
        let new = vec!["a", "NEW", "b"];
        let m = build_new_to_old_mapping(&old, &new);
        assert_eq!(m[1], None);
    }

    #[test]
    fn mixed_modify_and_add_in_same_hunk() {
        // old: ["a","b","c"]  new: ["a","X","Y","c"]
        // b→X is Modified, Y is purely Added
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "X", "Y", "c"];
        let m = build_new_to_old_mapping(&old, &new);
        assert_eq!(m[1], Some(1), "X maps to old 'b' (Modified)");
        assert_eq!(m[2], None, "Y has no old counterpart (Added)");
    }

    #[test]
    fn unchanged_file_maps_each_line_to_itself() {
        let lines = vec!["x", "y", "z"];
        let m = build_new_to_old_mapping(&lines, &lines);
        assert_eq!(m, vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn deleted_line_leaves_gap_in_old_idx_sequence() {
        let old = vec!["a", "b", "c"];
        let new = vec!["a", "c"];
        let m = build_new_to_old_mapping(&old, &new);
        assert_eq!(m[0], Some(0));
        assert_eq!(m[1], Some(2)); // "c" at old[2]; old[1]="b" is the gap
    }

    // ── missing_old_lines_before ─────────────────────────────────────────────

    #[test]
    fn finds_single_deleted_line() {
        let old = vec!["a", "b", "c"];
        let mapping = vec![Some(0), Some(2)]; // old[1]="b" is the gap
        let missing = missing_old_lines_before(1, &mapping, &old).unwrap();
        assert_eq!(missing, vec!["b"]);
    }

    #[test]
    fn finds_multiple_consecutive_deleted_lines() {
        let old = vec!["a", "b", "c", "d"];
        let mapping = vec![Some(0), Some(3)]; // old[1..3] = ["b","c"] are the gap
        let missing = missing_old_lines_before(1, &mapping, &old).unwrap();
        assert_eq!(missing, vec!["b", "c"]);
    }

    #[test]
    fn returns_none_when_no_gap() {
        let old = vec!["a", "b"];
        let mapping = vec![Some(0), Some(1)];
        assert!(missing_old_lines_before(1, &mapping, &old).is_none());
    }

    // ── buffer line helpers ──────────────────────────────────────────────────

    #[test]
    fn delete_buffer_line_removes_middle_line() {
        let mut buf = Buffer::new();
        buf.insert("line0\nline1\nline2\n");
        delete_buffer_line(&mut buf, 1);
        assert_eq!(buf.data(), "line0\nline2\n");
    }

    #[test]
    fn replace_buffer_line_replaces_content() {
        let mut buf = Buffer::new();
        buf.insert("line0\nold\nline2\n");
        replace_buffer_line(&mut buf, 1, "new");
        assert_eq!(buf.data(), "line0\nnew\nline2\n");
    }

    #[test]
    fn insert_buffer_line_before_inserts_correctly() {
        let mut buf = Buffer::new();
        buf.insert("line0\nline2\n");
        insert_buffer_line_before(&mut buf, 1, "line1");
        assert_eq!(buf.data(), "line0\nline1\nline2\n");
    }

    // ── end-to-end revert scenarios ──────────────────────────────────────────

    #[test]
    fn revert_added_line_by_deletion() {
        let mut buf = Buffer::new();
        buf.insert("a\nADDED\nb\n");
        delete_buffer_line(&mut buf, 1);
        assert_eq!(buf.data(), "a\nb\n");
    }

    #[test]
    fn revert_modified_line_by_replacement() {
        // This is the scenario the bug broke: mapping[1] was None so the
        // replace was silently skipped.
        let old = vec!["a", "ORIG", "b"];
        let new_buf_content = "a\nMOD\nb\n";
        let new_lines: Vec<&str> = new_buf_content.lines().collect();
        let mapping = build_new_to_old_mapping(&old, &new_lines);

        let mut buf = Buffer::new();
        buf.insert(new_buf_content);
        let old_idx = mapping[1].expect("Modified line must have old_idx after fix");
        replace_buffer_line(&mut buf, 1, old[old_idx]);
        assert_eq!(buf.data(), "a\nORIG\nb\n");
    }

    #[test]
    fn revert_deleted_line_by_insertion() {
        let old = vec!["a", "DELETED", "b"];
        let mapping = vec![Some(0), Some(2)];
        let missing = missing_old_lines_before(1, &mapping, &old).unwrap();

        let mut buf = Buffer::new();
        buf.insert("a\nb\n");
        for line in missing.iter().rev() {
            insert_buffer_line_before(&mut buf, 1, line);
        }
        assert_eq!(buf.data(), "a\nDELETED\nb\n");
    }

    #[test]
    fn revert_multiple_added_lines_in_reverse_order() {
        let mut buf = Buffer::new();
        buf.insert("a\nX\nY\nb\n");
        delete_buffer_line(&mut buf, 2); // higher index first
        delete_buffer_line(&mut buf, 1);
        assert_eq!(buf.data(), "a\nb\n");
    }
}

#[test]
fn test_get_gh_path() {
    let cases = [
        ("git@github.com:jmacdonald/amp.git", "jmacdonald/amp"),
        ("https://github.com/jmacdonald/amp.git", "jmacdonald/amp"),
        ("https://github.com/jmacdonald/amp", "jmacdonald/amp"),
    ];

    cases.iter().for_each(|(url, expected_gh_path)| {
        assert_eq!(&get_gh_path(url).unwrap(), expected_gh_path)
    })
}
