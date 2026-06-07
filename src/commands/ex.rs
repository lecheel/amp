use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::{Application, Mode};
use std::path::Path;

pub fn push_char(app: &mut Application) -> Result {
    let key = app.view.last_key().as_ref().context("No key press")?;
    if let crate::input::Key::Char(c) = *key {
        if let Mode::Ex(ref mut mode) = app.mode {
            mode.input.push(c);
            mode.update_completions(&app.workspace.path);
            // Auto-inline only for file completion (`:e ` prefix), not commands.
            if mode.input.trim_start_matches(':').starts_with("e ") {
                if mode.completions.len() == 1 {
                    mode.inline_complete();
                }
            }
        }
    }
    Ok(())
}

pub fn pop_char(app: &mut Application) -> Result {
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
        // No history fallback for left/right
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
    Ok(())
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

    // Parse and execute
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts.get(0).copied().unwrap_or("");
    let arg = parts.get(1).copied().unwrap_or("").trim();

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
    if let Mode::Ex(ref mut mode) = app.mode {
        if mode.completions.is_empty() {
            mode.update_completions(&app.workspace.path);
        }
        match mode.completions.len() {
            0 => {}
            1 => {
                // Single candidate: select it so it's visible in the popup,
                // user confirms with Enter or Tab again to apply.
                if mode.completion_selection == Some(0) {
                    // Already selected — second Tab applies it.
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
    Ok(())
}
