use crate::commands::{self, Result};
use crate::errors::*;
use crate::input::Key;
use crate::models::application::{Application, Mode};
use std::path::{Path, PathBuf};

pub fn push_char(app: &mut Application) -> Result {
    app.view.completion = None;
    let key = app.view.last_key().as_ref().context("No key press")?;
    if let Key::Char(c) = *key {
        if let Mode::Ex(ref mut mode) = app.mode {
            mode.input.push(c);
            mode.update_completions(&app.workspace.path);
            // Auto-inline only for file completion (`:e ` prefix), not commands.
            if mode.input.trim_start_matches(':').starts_with("e ") {
                if mode.completions.len() == 1 {
                    mode.inline_complete();
                    walk_into_directory(app)?;
                }
            }
        }
    }
    Ok(())
}

pub fn pop_char(app: &mut Application) -> Result {
    app.view.completion = None;
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.input.pop();
        mode.update_completions(&app.workspace.path);
    }
    Ok(())
}

// ── Smart navigation: popup when visible, history when not ──

pub fn navigate_up(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        if !mode.completions.is_empty() {
            mode.select_completion_up();
        } else {
            mode.history_previous();
        }
    }
    Ok(())
}

pub fn navigate_down(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        if !mode.completions.is_empty() {
            mode.select_completion_down();
        } else {
            mode.history_next();
        }
    }
    Ok(())
}

pub fn navigate_left(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        if !mode.completions.is_empty() {
            mode.select_completion_left();
        }
    }
    Ok(())
}

pub fn navigate_right(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        if !mode.completions.is_empty() {
            mode.select_completion_right();
        }
    }
    Ok(())
}

// ── Direct completion navigation (always navigates popup) ──

pub fn select_next_completion(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.select_next_completion();
    }
    Ok(())
}

pub fn select_previous_completion(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.select_previous_completion();
    }
    Ok(())
}

pub fn apply_completion(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.apply_selection();
    }
    walk_into_directory(app)
}

pub fn accept_input(app: &mut Application) -> Result {
    // If a completion is selected, apply it before executing
    if let Mode::Ex(ref mut mode) = app.mode {
        if mode.completion_selection.is_some() {
            mode.apply_selection();
        }
    }

    let input = if let Mode::Ex(ref mode) = app.mode {
        mode.input.trim_start_matches(':').trim().to_string()
    } else {
        bail!("Not in ex mode");
    };

    // Parse early so we can intercept :e on a directory
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts.get(0).copied().unwrap_or("");
    let arg = parts.get(1).copied().unwrap_or("").trim();

    // For :e, if the resolved path is a directory, walk into it
    // instead of trying (and failing) to open it as a file.
    if cmd == "e" && !arg.is_empty() {
        let path = Path::new(arg);
        let full_path = if path.is_absolute() {
            PathBuf::from(path)
        } else {
            app.workspace.path.join(path)
        };

        if full_path.is_dir() {
            if let Mode::Ex(ref mut mode) = app.mode {
                let clean_arg = arg.trim_end();
                // Ensure the input has a trailing slash
                if !clean_arg.ends_with('/') {
                    mode.input = format!(":e {}/", clean_arg);
                }
                // Always refresh — completions for the parent prefix
                // are stale now that we're inside the directory.
                mode.completion_selection = None;
                mode.update_completions(&app.workspace.path);
            }
            return Ok(());
        }
    }

    // Save to history
    if let Mode::Ex(ref mut mode) = app.mode {
        if !input.is_empty() {
            mode.history.push(format!(":{}", input));
            mode.history_index = mode.history.len();
        }
        mode.input.clear();
        mode.completions.clear();
        mode.completion_selection = None;
    }

    // Execute
    match cmd {
        "q" => commands::application::exit(app)?,
        "q!" => commands::alias::force_exit(app)?,
        "w" => commands::buffer::save(app)?,
        "wq" => commands::alias::save_and_exit(app)?,
        "bn" => commands::workspace::next_buffer(app)?,
        "bp" => commands::workspace::prev_buffer(app)?,
        "bd" => commands::buffer::close(app)?,
        "ls" => commands::application::switch_to_buffer_list_mode(app)?,
        "rg" => commands::rg::search(app, arg)?,
        "cn" => commands::rg::next_result(app)?,
        "cp" => commands::rg::prev_result(app)?,
        "fd" => commands::fd::switch_to_fd_mode(app, arg)?,
        "gentags" => commands::tag::gentags(app)?,
        "tag" | "ta" => {
            if arg.is_empty() {
                bail!("No tag specified for :tag. Usage: :tag <name>");
            } else {
                commands::tag::tag(app, arg)?;
            }
        }
        "last_rg" => commands::rg::switch_to_last_rg(app)?,
        "e" => {
            if arg.is_empty() {
                bail!("No file specified for :e");
            } else {
                let path = Path::new(arg);
                crate::util::open_buffer(path, app)?;
            }
        }
        _ => bail!("Unknown command: {}", cmd),
    }

    if matches!(app.mode, Mode::Ex(_)) {
        commands::application::switch_to_normal_mode(app)?;
    }

    Ok(())
}

// ── Direct history navigation (always navigates history) ──

pub fn previous_history(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.history_previous();
    }
    Ok(())
}

pub fn next_history(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.history_next();
    }
    Ok(())
}

pub fn complete(app: &mut Application) -> Result {
    app.view.completion = None;
    if let Mode::Ex(ref mut mode) = app.mode {
        if mode.completions.is_empty() {
            mode.update_completions(&app.workspace.path);
        }
        match mode.completions.len() {
            0 => {}
            1 => {
                if mode.completion_selection == Some(0) {
                    mode.apply_selection();
                } else {
                    mode.completion_selection = Some(0);
                }
            }
            _ => {
                mode.select_next_completion();
            }
        }
    }
    walk_into_directory(app)
}

/// After a completion is applied for the `:e` command, check if the
/// resulting path is a directory. If so, strip any trailing whitespace
/// that `apply_selection`/`inline_complete` may have added, append `/`
/// if missing, and ALWAYS refresh completions so the popup shows the
/// directory's contents.
fn walk_into_directory(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        let trimmed_input = mode.input.trim_start_matches(':');
        if let Some(path_part) = trimmed_input.strip_prefix("e ") {
            // trim_end() removes the trailing space that
            // apply_selection/inline_complete insert after a completed
            // path.  Without this we'd produce ":e src /" instead of
            // ":e src/".
            let path_part = path_part.trim_end();
            let full_path = if Path::new(path_part).is_absolute() {
                PathBuf::from(path_part)
            } else {
                app.workspace.path.join(path_part)
            };
            if full_path.is_dir() {
                // Ensure trailing slash
                if !path_part.ends_with('/') {
                    mode.input = format!(":e {}/", path_part);
                }
                // Always refresh completions for the directory contents,
                // even when the path already ends with '/'.  The previous
                // completions (matching the parent prefix like "src") are
                // stale once we've stepped inside the directory.
                mode.completion_selection = None;
                mode.update_completions(&app.workspace.path);
            }
        }
    }
    Ok(())
}

pub fn complete_from_buffer(app: &mut Application) -> Result {
    let buffer_data = app
        .workspace
        .current_buffer
        .as_ref()
        .map(|b| b.data())
        .unwrap_or_default();

    if let Mode::Ex(ref mut mode) = app.mode {
        mode.generate_buffer_completions(&buffer_data);

        // Oneshot: single match → apply immediately, no popup needed.
        if mode.completions.len() == 1 {
            mode.apply_selection();
        }
    } else {
        bail!("Can't complete from buffer outside of ex mode");
    }

    Ok(())
}
