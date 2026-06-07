use std::path::Path;

#[derive(Default)]
pub struct ExMode {
    pub input: String,
    pub history: Vec<String>,
    pub history_index: usize,

    // NEW: completion state
    pub completions: Vec<String>,
    pub completion_selection: Option<usize>, // None = no selection
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

    /// Recalculate completions based on current input
    pub fn update_completions(&mut self, workspace_path: &Path) {
        self.completions = self.generate_completions(workspace_path);
        // Reset selection if completions changed
        if self.completions.is_empty() {
            self.completion_selection = None;
        } else if let Some(idx) = self.completion_selection {
            if idx >= self.completions.len() {
                self.completion_selection = Some(0);
            }
        }
    }

    fn generate_completions(&self, workspace_path: &Path) -> Vec<String> {
        let input = self.input.trim_start_matches(':');
        let mut results = Vec::new();

        if input.starts_with("e ") {
            // File path completion
            let prefix = input.splitn(2, ' ').nth(1).unwrap_or("");
            if let Ok(entries) = std::fs::read_dir(workspace_path) {
                for entry in entries.flatten() {
                    if let Some(name) = entry.file_name().to_str() {
                        if name.starts_with(prefix) {
                            results.push(format!(":e {}", name));
                        }
                    }
                }
            }
            results.sort();
        } else {
            // Command completion
            let commands = [":q", ":q!", ":w", ":wq", ":bn", ":bp", ":bd", ":e", ":ls"];
            for cmd in &commands {
                if cmd.starts_with(&self.input) || cmd.starts_with(&format!(":{}", input)) {
                    results.push(cmd.to_string());
                }
            }
            results.sort();
            results.dedup();
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
                    Some(0) // wrap around
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
                    Some(self.completions.len() - 1) // wrap around
                }
            }
            None => Some(self.completions.len() - 1),
        };
    }

    pub fn apply_selection(&mut self) {
        if let Some(idx) = self.completion_selection {
            if let Some(completion) = self.completions.get(idx).cloned() {
                self.input = completion;
                // Add trailing space for :e to make typing path easier
                if self.input.starts_with(":e ") && !self.input.ends_with(' ') {
                    self.input.push(' ');
                }
                self.completions.clear();
                self.completion_selection = None;
            }
        }
    }
}
