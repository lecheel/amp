use crate::commands::{self, Result};
use crate::errors::*;
use crate::models::application::{Application, Mode};
use std::path::Path;

pub fn push_char(app: &mut Application) -> Result {
    let key = app.view.last_key().as_ref().context("No key press")?;
    if let crate::input::Key::Char(c) = *key {
        if let Mode::Ex(ref mut mode) = app.mode {
            mode.input.push(c);
        }
    }
    Ok(())
}

pub fn pop_char(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        mode.input.pop();
    }
    Ok(())
}

pub fn accept_input(app: &mut Application) -> Result {
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
    }

    // Parse and execute
    let parts: Vec<&str> = input.splitn(2, ' ').collect();
    let cmd = parts.get(0).copied().unwrap_or("");
    let arg = parts.get(1).copied().unwrap_or("").trim();

    match cmd {
        "q" => commands::application::exit(app)?,
        "w" => commands::buffer::save(app)?,
        "bn" => commands::workspace::next_buffer(app)?,
        "bp" => commands::workspace::prev_buffer(app)?,
        "bd" => commands::buffer::close(app)?,
        "ls" => commands::application::switch_to_buffer_list_mode(app)?,
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

    commands::application::switch_to_normal_mode(app)
}

pub fn previous_history(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        if mode.history_index > 0 {
            mode.history_index -= 1;
            if let Some(entry) = mode.history.get(mode.history_index) {
                mode.input = entry.clone();
            }
        }
    }
    Ok(())
}

pub fn next_history(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        if mode.history_index < mode.history.len() {
            mode.history_index += 1;
            if let Some(entry) = mode.history.get(mode.history_index) {
                mode.input = entry.clone();
            } else {
                mode.input.clear();
            }
        }
    }
    Ok(())
}

pub fn complete(app: &mut Application) -> Result {
    if let Mode::Ex(ref mut mode) = app.mode {
        let input = mode.input.trim_start_matches(':').to_string();

        if input.starts_with("e ") {
            // File path completion
            let prefix = input.splitn(2, ' ').nth(1).unwrap_or("");
            let mut matches = Vec::new();

            if let Ok(entries) = std::fs::read_dir(&app.workspace.path) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with(prefix) {
                            matches.push(name.to_string());
                        }
                    }
                }
            }

            if matches.len() == 1 {
                mode.input = format!(":e {} ", matches[0]);
            }
        } else {
            // Command completion
            let commands = [":q", ":q!", ":w", ":wq", ":bn", ":bp", ":bd", ":e", ":ls"];
            let matches: Vec<_> = commands
                .iter()
                .filter(|c| c.starts_with(&mode.input))
                .collect();

            if matches.len() == 1 {
                mode.input = matches[0].to_string();
                if !mode.input.ends_with(' ') && !mode.input.ends_with('!') {
                    mode.input.push(' ');
                }
            }
        }
    }
    Ok(())
}
