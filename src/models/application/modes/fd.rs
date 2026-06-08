use crate::models::application::modes::open::DisplayablePath;
use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use std::fmt;
use std::path::PathBuf;
use std::process::Command;
use std::slice::Iter;

pub struct FdMode {
    insert: bool,
    input: String,
    paths: Vec<DisplayablePath>,
    results: SelectableVec<DisplayablePath>,
    config: SearchSelectConfig,
}

impl FdMode {
    pub fn new(config: SearchSelectConfig) -> FdMode {
        FdMode {
            insert: true,
            input: String::new(),
            paths: Vec::new(),
            results: SelectableVec::new(Vec::new()),
            config,
        }
    }

    pub fn reset(&mut self, workspace_path: &PathBuf, config: SearchSelectConfig, filter: &str) {
        self.input.clear();
        self.insert = true;
        self.config = config;

        // Build fd command — pass filter as a positional pattern argument
        // so fd does the initial filtering server-side for efficiency.
        let mut cmd = Command::new("fd");
        cmd.args(["--type", "f", "--follow"]);

        if !filter.is_empty() {
            cmd.arg(filter);
        }

        self.paths = match cmd.current_dir(workspace_path).output() {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout)
                    .lines()
                    .filter(|l| !l.is_empty())
                    .map(|l| DisplayablePath(workspace_path.join(l))) // ← was PathBuf::from(l)
                    .collect()
            }
            _ => Vec::new(),
        };

        // Pre-populate the query so the user can refine further
        if !filter.is_empty() {
            self.input.push_str(filter);
        }

        self.results = SelectableVec::new(
            self.paths
                .iter()
                .take(self.config.max_results)
                .cloned()
                .collect(),
        );
    }
}

impl fmt::Display for FdMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "FD")
    }
}

impl SearchSelectMode for FdMode {
    type Item = DisplayablePath;

    fn search(&mut self) {
        if self.input.is_empty() {
            self.results = SelectableVec::new(
                self.paths
                    .iter()
                    .take(self.config.max_results)
                    .cloned()
                    .collect(),
            );
        } else {
            let query = self.input.to_lowercase();
            let results: Vec<DisplayablePath> = self
                .paths
                .iter()
                .filter(|p| p.0.to_string_lossy().to_lowercase().contains(&query))
                .take(self.config.max_results)
                .cloned()
                .collect();
            self.results = SelectableVec::new(results);
        }
    }

    fn query(&mut self) -> &mut String {
        &mut self.input
    }

    fn insert_mode(&self) -> bool {
        self.insert
    }

    fn set_insert_mode(&mut self, insert_mode: bool) {
        self.insert = insert_mode;
    }

    fn results(&self) -> Iter<'_, DisplayablePath> {
        self.results.iter()
    }

    fn selection(&self) -> Option<&DisplayablePath> {
        self.results.selection()
    }

    fn selected_index(&self) -> usize {
        self.results.selected_index()
    }

    fn select_previous(&mut self) {
        self.results.select_previous();
    }

    fn select_next(&mut self) {
        self.results.select_next();
    }

    fn config(&self) -> &SearchSelectConfig {
        &self.config
    }

    fn message(&mut self) -> Option<String> {
        if self.paths.is_empty() {
            Some(String::from("No files found (is 'fd' installed?)"))
        } else if !self.query().is_empty() && self.results().count() == 0 {
            Some(String::from("No matching entries found."))
        } else {
            None
        }
    }
}
