use std::collections::BTreeSet;
use std::path::Path;

const COMPLETION_COLUMNS: usize = 4;

#[derive(Clone, Debug)]
pub struct CompletionEntry {
    pub display: String,
    pub value: String,
}

#[derive(Default)]
pub struct ExMode {
    pub input: String,
    pub history: Vec<String>,
    pub history_index: usize,
    pub completions: Vec<CompletionEntry>,
    pub completion_selection: Option<usize>,
}

impl ExMode {
    pub fn new() -> ExMode {
        ExMode::default()
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.history_index = self.history.len();
        self.completions.clear();
        self.completion_selection = None;
    }

    pub fn update_completions(&mut self, workspace_path: &Path) {
        self.completions = self.generate_completions(workspace_path);
        if self.completions.is_empty() {
            self.completion_selection = None;
        } else {
            match self.completion_selection {
                // Keep current selection if it's still valid
                Some(idx) if idx < self.completions.len() => {}
                // No selection or out of bounds — select the first item
                _ => self.completion_selection = Some(0),
            }
        }
    }

    fn generate_completions(&self, workspace_path: &Path) -> Vec<CompletionEntry> {
        let input = self.input.trim_start_matches(':');
        let mut results = Vec::new();

        if input.starts_with("e ") {
            let prefix = input.splitn(2, ' ').nth(1).unwrap_or("");

            // Split into the directory to read and the filename prefix to filter by.
            // "src/"        -> dir="src/",  file_prefix=""
            // "src/mo"      -> dir="src/",  file_prefix="mo"
            // "Cargo"       -> dir="",      file_prefix="Cargo"
            let (dir_part, file_prefix) = match prefix.rfind('/') {
                Some(slash) => (&prefix[..=slash], &prefix[slash + 1..]),
                None => ("", prefix),
            };

            let search_dir = if dir_part.is_empty() {
                workspace_path.to_path_buf()
            } else {
                let p = Path::new(dir_part);
                if p.is_absolute() {
                    p.to_path_buf()
                } else {
                    workspace_path.join(dir_part)
                }
            };

            if let Ok(entries) = std::fs::read_dir(&search_dir) {
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                    .filter(|name| name.starts_with(file_prefix))
                    .collect();
                names.sort();

                for name in names {
                    let full_rel = format!("{}{}", dir_part, name);
                    let is_dir = workspace_path.join(&full_rel).is_dir();
                    let display = if is_dir {
                        format!("{}/", name) // Show only the item name, not the dir prefix
                    } else {
                        name.clone() // Show only the item name
                    };
                    // Trailing space signals "complete, ready to accept"
                    let value = format!(":e {} ", full_rel);
                    results.push(CompletionEntry { display, value });
                }
            }
        } else {
            let commands = [
                (":q", ":q"),
                (":q!", ":q!"),
                (":w", ":w"),
                (":wq", ":wq"),
                (":bn", ":bn"),
                (":bp", ":bp"),
                (":bd", ":bd"),
                (":e ", ":e "),
                (":ls", ":ls"),
                (":rg ", ":rg "),
                (":last_rg", ":last_rg"),
                (":tag ", ":tag "),
                (":gentags", ":gentags"),
                (":sed ", ":sed "),
                (":sed -w ", ":sed -w "),
                (":cn", ":cn"),
                (":cp", ":cp"),
            ];
            for (display, value) in &commands {
                let matches = self.input.starts_with(':') && display.starts_with(&self.input);
                let matches_no_colon = !self.input.starts_with(':')
                    && display.trim_start_matches(':').starts_with(input);
                if matches || matches_no_colon {
                    results.push(CompletionEntry {
                        display: display.to_string(),
                        value: value.to_string(),
                    });
                }
            }
            results.sort_by(|a, b| a.display.cmp(&b.display));
            results.dedup_by(|a, b| a.display == b.display);
        }

        results
    }

    // ── Grid navigation ──────────────────────────────────
    pub fn select_completion_down(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let current = self.completion_selection.unwrap_or(0);
        let below = current + COMPLETION_COLUMNS;
        if below < self.completions.len() {
            self.completion_selection = Some(below);
        }
    }

    pub fn select_completion_up(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let current = self.completion_selection.unwrap_or(0);
        if current >= COMPLETION_COLUMNS {
            self.completion_selection = Some(current - COMPLETION_COLUMNS);
        }
    }

    pub fn select_completion_right(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let current = self.completion_selection.unwrap_or(0);
        let current_row = current / COMPLETION_COLUMNS;
        let next_in_row = current + 1;
        if next_in_row / COMPLETION_COLUMNS == current_row && next_in_row < self.completions.len() {
            self.completion_selection = Some(next_in_row);
        }
    }

    pub fn select_completion_left(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        let current = self.completion_selection.unwrap_or(0);
        if current % COMPLETION_COLUMNS > 0 {
            self.completion_selection = Some(current - 1);
        }
    }

    // ── Sequential navigation (Tab / Ctrl-N / Ctrl-P) ────
    pub fn select_next_completion(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        self.completion_selection = match self.completion_selection {
            Some(idx) => {
                if idx + 1 < self.completions.len() {
                    Some(idx + 1)
                } else {
                    Some(0)
                }
            }
            None => Some(0),
        };
    }

    pub fn select_previous_completion(&mut self) {
        if self.completions.is_empty() {
            return;
        }
        self.completion_selection = match self.completion_selection {
            Some(idx) => {
                if idx > 0 {
                    Some(idx - 1)
                } else {
                    Some(self.completions.len() - 1)
                }
            }
            None => Some(self.completions.len() - 1),
        };
    }

    // ── History navigation ────────────────────────────────
    pub fn history_previous(&mut self) {
        if self.history_index > 0 {
            self.history_index -= 1;
            if let Some(entry) = self.history.get(self.history_index) {
                self.input = entry.clone();
            }
        }
    }

    pub fn history_next(&mut self) {
        if self.history_index < self.history.len() {
            self.history_index += 1;
            if let Some(entry) = self.history.get(self.history_index) {
                self.input = entry.clone();
            } else {
                self.input.clear();
            }
        }
    }

    /// Inline-complete the unambiguous suffix into `self.input`.
    /// Called after `update_completions` when exactly one candidate exists.
    pub fn inline_complete(&mut self) {
        if self.completions.len() == 1 {
            self.input = self.completions[0].value.clone();
        }
    }

    // ── Apply selection ───────────────────────────────────
    pub fn apply_selection(&mut self) {
        if let Some(idx) = self.completion_selection {
            if let Some(entry) = self.completions.get(idx).cloned() {
                self.input = entry.value;
                self.completions.clear();
                self.completion_selection = None;
            }
        }
    }

    /// Generate completions from words in the current buffer that match the
    /// alphanumeric/underscore prefix at the end of the ex input line.
    /// Uses the same `completions` / `completion_selection` fields as
    /// command/file completions so the popup UI is unified.
    pub fn generate_buffer_completions(&mut self, buffer_data: &str) {
        let input = self.input.trim_start_matches(':');

        // Walk backwards to find where the trailing "word" (alphanum + '_') starts.
        let mut prefix_start = input.len();
        for (i, c) in input.char_indices().rev() {
            if c.is_alphanumeric() || c == '_' {
                prefix_start = i;
            } else {
                break;
            }
        }

        let prefix = &input[prefix_start..];

        if prefix.is_empty() {
            self.completions.clear();
            self.completion_selection = None;
            return;
        }

        let prefix_lower = prefix.to_lowercase();
        let mut word_set = BTreeSet::new();

        for line in buffer_data.lines() {
            for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if word.len() > prefix.len() && word.to_lowercase().starts_with(&prefix_lower) {
                    word_set.insert(word.to_string());
                }
            }
        }

        let before = &input[..prefix_start];

        self.completions = word_set
            .into_iter()
            .take(100)
            .map(|word| CompletionEntry {
                display: word.clone(),
                value: format!(":{}{}", before, word),
            })
            .collect();

        if self.completions.is_empty() {
            self.completion_selection = None;
        } else {
            self.completion_selection = Some(0);
        }
    }
}
