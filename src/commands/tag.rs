use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::ctagd;
use crate::models::application::modes::tag_jump::TagEntry;
use crate::models::application::modes::SearchSelectMode;
use crate::models::application::{Application, Mode, ModeKey};
use scribe::buffer::Position;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Get the git repo root for ctagd requests, or None if not in a git repo.
fn repo_root(app: &Application) -> Option<PathBuf> {
    app.repository
        .as_ref()
        .and_then(|repo| repo.workdir().map(|p| p.to_path_buf()))
}

/// Jump to a tag by name. Uses ctagd `workspace_symbols` if available,
/// otherwise falls back to reading the ctags file from disk.
pub fn tag(app: &mut Application, query: &str) -> Result {
    let tags = if app.ctagd_available {
        let root = match repo_root(app) {
            Some(r) => r,
            None => {
                app.ctagd_available = false;
                return tag_fallback(app, query);
            }
        };
        match ctagd::workspace_symbols(&root, query) {
            Ok(symbols) => symbols
                .into_iter()
                .map(|s| TagEntry {
                    name: s.name,
                    // ctagd paths are relative to repo root, not workspace
                    file: root.join(s.relative_path),
                    line: s.line,
                    kind: s.kind,
                })
                .collect(),
            Err(e) => {
                log::debug!(
                    "ctagd workspace_symbols failed, falling back to tags file: {}",
                    e
                );
                app.ctagd_available = false;
                return tag_fallback(app, query);
            }
        }
    } else {
        read_tags_from_file(&app.workspace.path)?
    };

    if tags.is_empty() {
        bail!(
            "No tags found for '{}'. Try :gentags to generate a tags file.",
            query
        );
    }

    app.switch_to(ModeKey::TagJump);
    let config = app.preferences.borrow().search_select_config();
    match app.mode {
        Mode::TagJump(ref mut mode) => {
            mode.reset(tags, config);
            mode.query().push_str(query);
        }
        _ => bail!("Expected TagJump mode"),
    }
    commands::search_select::search(app)?;

    let single_result = match app.mode {
        Mode::TagJump(ref mut mode) => mode.results().count() == 1,
        _ => false,
    };
    if single_result {
        commands::search_select::accept(app)?;
    }

    Ok(())
}

/// Fallback: read tags from file and enter TagJump mode.
fn tag_fallback(app: &mut Application, query: &str) -> Result {
    let tags = read_tags_from_file(&app.workspace.path)?;
    if tags.is_empty() {
        bail!(
            "No tags found for '{}'. Try :gentags to generate a tags file.",
            query
        );
    }

    app.switch_to(ModeKey::TagJump);
    let config = app.preferences.borrow().search_select_config();
    match app.mode {
        Mode::TagJump(ref mut mode) => {
            mode.reset(tags, config);
            mode.query().push_str(query);
        }
        _ => bail!("Expected TagJump mode"),
    }
    commands::search_select::search(app)?;

    let single_result = match app.mode {
        Mode::TagJump(ref mut mode) => mode.results().count() == 1,
        _ => false,
    };
    if single_result {
        commands::search_select::accept(app)?;
    }

    Ok(())
}

/// Jump to the tag under the cursor. Uses ctagd `definition` if available
/// for precise LSP-based go-to-definition, otherwise falls back to
/// `tag()` which searches the tags file.
pub fn tag_under_cursor(app: &mut Application) -> Result {
    let (word, relative_path, cursor_line, cursor_offset) = {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .context(BUFFER_MISSING)?;
        let cursor = *buffer.cursor;
        let data = buffer.data();
        let line_text = data
            .lines()
            .nth(cursor.line)
            .context(CURRENT_LINE_MISSING)?;
        let word =
            extract_word_at(line_text, cursor.offset).context("No tag found under cursor")?;

        // Resolve relative path against git repo root
        let root = repo_root(app).unwrap_or_else(|| app.workspace.path.clone());
        let path = buffer.path.as_ref().context(BUFFER_PATH_MISSING)?;
        let relative = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        (word, relative, cursor.line, cursor.offset)
    };

    if app.ctagd_available {
        if let Some(root) = repo_root(app) {
            match ctagd::definition(&root, &relative_path, cursor_line, cursor_offset, &word) {
                Ok(defs) if !defs.is_empty() => {
                    if defs.len() == 1 {
                        let def = &defs[0];

                        if let Some(buf) = app.workspace.current_buffer.as_ref() {
                            if let Some(p) = buf.path.clone() {
                                app.tag_jump_stack.push((p, *buf.cursor));
                            }
                        }

                        // ctagd paths are relative to repo root
                        let target_path = root.join(&def.file);
                        crate::util::open_buffer(&target_path, app)?;

                        if let Some(buf) = app.workspace.current_buffer.as_mut() {
                            let target_line = def.line.min(buf.line_count().saturating_sub(1));
                            let target_col = def.column.min(
                                buf.data()
                                    .lines()
                                    .nth(target_line)
                                    .map(|l| l.len())
                                    .unwrap_or(0),
                            );
                            buf.cursor.move_to(Position {
                                line: target_line,
                                offset: target_col,
                            });
                        }

                        commands::view::scroll_cursor_to_center(app)?;
                        return Ok(());
                    } else {
                        // Multiple definitions — use TagJump selection UI
                        let tags: Vec<TagEntry> = defs
                            .iter()
                            .map(|def| TagEntry {
                                name: word.clone(),
                                // ctagd paths are relative to repo root
                                file: root.join(&def.file),
                                line: def.line,
                                kind: def.display.clone().or_else(|| Some(word.clone())),
                            })
                            .collect();

                        app.switch_to(ModeKey::TagJump);
                        let config = app.preferences.borrow().search_select_config();
                        match app.mode {
                            Mode::TagJump(ref mut mode) => {
                                mode.reset(tags, config);
                                mode.query().push_str(&word);
                            }
                            _ => bail!("Expected TagJump mode"),
                        }
                        commands::search_select::search(app)?;
                        return Ok(());
                    }
                }
                Ok(_) => {
                    // Empty results — fall through to tag search
                }
                Err(e) => {
                    log::debug!("ctagd definition failed, falling back to tag search: {}", e);
                    app.ctagd_available = false;
                }
            }
        }
    }

    // Fallback: use tag() which searches ctags file with selection UI
    tag(app, &word)
}

/// Generate a tags file using universal-ctags in the repo root.
/// Outputs to ./tags at the repo root (same level as .git/).
pub fn gentags(app: &mut Application) -> Result {
    let workspace_path = app.workspace.path.clone();
    let output_path = workspace_path.join("tags");

    let output = Command::new("ctags")
        .args([
            "-R",
            "--excmd=number",
            "--fields=+K",
            "-o",
            output_path.to_str().context("Invalid tags output path")?,
            ".",
        ])
        .current_dir(&workspace_path)
        .output()
        .context("Failed to run ctags. Is universal-ctags installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("ctags failed: {}", stderr.trim());
    }

    if output_path.exists() {
        Ok(())
    } else {
        bail!(
            "ctags completed but no tags file found at {}",
            output_path.display()
        );
    }
}

/// Jump back to the previous position before the last tag jump.
pub fn tag_back(app: &mut Application) -> Result {
    let (path, position) = app
        .tag_jump_stack
        .pop()
        .context("No previous tag jump to return to")?;

    crate::util::open_buffer(&path, app)?;

    if let Some(buf) = app.workspace.current_buffer.as_mut() {
        let ln = position.line.min(buf.line_count().saturating_sub(1));
        let offset = position
            .offset
            .min(buf.data().lines().nth(ln).map(|l| l.len()).unwrap_or(0));
        buf.cursor.move_to(Position { line: ln, offset });
    }

    commands::view::scroll_cursor_to_center(app)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// ctags file fallback
// ---------------------------------------------------------------------------

fn read_tags_from_file(workspace_path: &Path) -> anyhow::Result<Vec<TagEntry>> {
    let candidates = [
        workspace_path.join("tags"),
        workspace_path.join(".git/tags"),
        workspace_path.join(".tags"),
    ];

    let tags_path = candidates
        .iter()
        .find(|p| p.exists())
        .context("No tags file found. Try :gentags to generate one.")?;

    let content = fs::read_to_string(tags_path).context("Failed to read tags file")?;
    Ok(parse_tags(&content, workspace_path))
}

fn parse_tags(content: &str, workspace_path: &Path) -> Vec<TagEntry> {
    content
        .lines()
        .filter(|l| !l.starts_with("!_TAG"))
        .filter_map(|l| parse_tag_line(l, workspace_path))
        .collect()
}

fn parse_tag_line(line: &str, workspace_path: &Path) -> Option<TagEntry> {
    let parts: Vec<&str> = line.split('\t').collect();
    if parts.len() < 3 {
        return None;
    }

    let name = parts[0].to_string();
    let file = if Path::new(parts[1]).is_absolute() {
        PathBuf::from(parts[1])
    } else {
        workspace_path.join(parts[1])
    };

    let address = parts[2];
    let line_num = parse_address(address)?;

    let kind = parts[3..]
        .iter()
        .find(|f| f.starts_with("kind:") || f.starts_with("kind\x07"))
        .map(|f| {
            let sep_len = if f.starts_with("kind:") { 5 } else { 5 };
            f[sep_len..].to_string()
        })
        .or_else(|| {
            parts
                .iter()
                .skip(3)
                .find(|f| {
                    !f.starts_with("kind:")
                        && !f.starts_with("line:")
                        && !f.starts_with("language:")
                        && f.len() == 1
                        && f.chars().next().map_or(false, |c| c.is_alphabetic())
                })
                .map(|f| f.to_string())
        });

    Some(TagEntry {
        name,
        file,
        line: line_num.saturating_sub(1),
        kind,
    })
}

fn parse_address(address: &str) -> Option<usize> {
    if let Ok(num) = address.parse::<usize>() {
        return Some(num);
    }
    if let Some(slash_pos) = address.find(';') {
        if let Ok(num) = address[..slash_pos].parse::<usize>() {
            return Some(num);
        }
    }
    None
}

fn extract_word_at(line: &str, offset: usize) -> Option<String> {
    let chars: Vec<char> = line.chars().collect();
    if offset >= chars.len() {
        return None;
    }
    let start_offset = if is_word_char(chars[offset]) {
        offset
    } else if offset > 0 && is_word_char(chars[offset - 1]) {
        offset - 1
    } else {
        return None;
    };
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
