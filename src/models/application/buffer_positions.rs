use crate::errors::*;
use crate::models::application::Preferences;
use scribe::buffer::Position;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const FILE_NAME: &str = "buffer_positions";
const MAX_ENTRIES: usize = 100;

#[derive(Clone, Debug)]
pub struct BufferPosition {
    pub position: Position,
    pub last_used: SystemTime,
}

pub type PositionMap = HashMap<String, BufferPosition>;

pub fn load() -> Result<PositionMap> {
    let path = file_path()?;
    if !path.exists() {
        return Ok(HashMap::new());
    }

    let content = fs::read_to_string(&path)?;
    let mut map = HashMap::new();

    for line in content.lines() {
        let parts: Vec<&str> = line.splitn(4, ':').collect();

        if parts.len() == 4 {
            // New format: epoch_secs:line:offset:/path/to/file
            if let (Ok(epoch), Ok(line_no), Ok(offset)) = (
                parts[0].parse::<u64>(),
                parts[1].parse::<usize>(),
                parts[2].parse::<usize>(),
            ) {
                let path_str = parts[3].to_string();
                if !path_str.starts_with('[') && !path_str.is_empty() {
                    map.insert(
                        path_str,
                        BufferPosition {
                            position: Position {
                                line: line_no,
                                offset,
                            },
                            last_used: UNIX_EPOCH + Duration::from_secs(epoch),
                        },
                    );
                }
            }
        } else if parts.len() == 3 {
            // Old format fallback: line:offset:/path/to/file
            if let (Ok(line_no), Ok(offset)) =
                (parts[0].parse::<usize>(), parts[1].parse::<usize>())
            {
                let path_str = parts[2].to_string();
                if !path_str.starts_with('[') && !path_str.is_empty() {
                    map.insert(
                        path_str,
                        BufferPosition {
                            position: Position {
                                line: line_no,
                                offset,
                            },
                            // Assign epoch 0 so old entries get naturally pushed out by new ones
                            last_used: UNIX_EPOCH,
                        },
                    );
                }
            }
        }
    }

    Ok(map)
}

pub fn save(map: &PositionMap) -> Result<()> {
    let path = file_path()?;

    // Sort by last_used descending (most recent first)
    let mut entries: Vec<_> = map.iter().collect();
    entries.sort_by(|a, b| b.1.last_used.cmp(&a.1.last_used));

    // Housekeeping: keep only the 100 most recent entries
    entries.truncate(MAX_ENTRIES);

    let mut content = String::new();
    for (file_path, buf_pos) in entries {
        let epoch = buf_pos
            .last_used
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        content.push_str(&format!(
            "{}:{}:{}:{}\n",
            epoch, buf_pos.position.line, buf_pos.position.offset, file_path
        ));
    }

    fs::write(&path, content)?;
    Ok(())
}

fn file_path() -> Result<PathBuf> {
    Ok(Preferences::directory()?.join(FILE_NAME))
}
