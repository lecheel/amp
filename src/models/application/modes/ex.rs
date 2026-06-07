use std::path::Path;

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
        } else if let Some(idx) = self.completion_selection {
            if idx >= self.completions.len() {
                self.completion_selection = Some(0);
            }
        }
    }

    fn generate_completions(&self, workspace_path: &Path) -> Vec<CompletionEntry> {
        let input = self.input.trim_start_matches(':');
        let mut results = Vec::new();

        if input.starts_with("e ") {
            // File path completion — show only the filename
            let prefix = input.splitn(2, ' ').nth(1).unwrap_or("");
            if let Ok(entries) = std::fs::read_dir(workspace_path) {
                let mut names: Vec<String> = entries
                    .flatten()
                    .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
                    .filter(|name| name.starts_with(prefix))
                    .collect();
                names.sort();

                for name in names {
                    let is_dir = workspace_path.join(&name).is_dir();
                    let display = if is_dir {
                        format!("{}/", name)
                    } else {
                        name.clone()
                    };
                    let value = format!(":e {} ", name);

                    results.push(CompletionEntry { display, value });
                }
            }
        } else {
            // Command completion — display and value are the same
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

    pub fn apply_selection(&mut self) {
        if let Some(idx) = self.completion_selection {
            if let Some(entry) = self.completions.get(idx).cloned() {
                self.input = entry.value;
                self.completions.clear();
                self.completion_selection = None;
            }
        }
    }
}
