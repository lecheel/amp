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
    paths: Vec<DisplayablePath>,             // ALL files from fd
    all_results: Vec<DisplayablePath>,       // ALL matches from search (not truncated)
    scroll_offset: usize,                    // index into all_results of first visible item
    results: SelectableVec<DisplayablePath>, // visible window only
    config: SearchSelectConfig,
}

impl FdMode {
    pub fn new(config: SearchSelectConfig) -> FdMode {
        FdMode {
            insert: true,
            input: String::new(),
            paths: Vec::new(),
            all_results: Vec::new(),
            scroll_offset: 0,
            results: SelectableVec::new(Vec::new()),
            config,
        }
    }

    pub fn reset(&mut self, workspace_path: &PathBuf, config: SearchSelectConfig, filter: &str) {
        self.input.clear();
        self.insert = true;
        self.config = config;
        self.scroll_offset = 0;

        let mut cmd = Command::new("fd");
        cmd.args(["--type", "f", "--follow"]);

        if !filter.is_empty() {
            cmd.arg(filter);
        }

        self.paths = match cmd.current_dir(workspace_path).output() {
            Ok(output) if output.status.success() => String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .map(|l| DisplayablePath(workspace_path.join(l)))
                .collect(),
            _ => Vec::new(),
        };

        if !filter.is_empty() {
            self.input.push_str(filter);
        }

        self.all_results = self.paths.clone();
        self.update_visible_results(0);
    }

    /// Recompute the visible window from all_results[scroll_offset..]
    /// and set the cursor to `cursor_index` within the visible window.
    fn update_visible_results(&mut self, cursor_index: usize) {
        let max = self.config.max_results;
        let end = (self.scroll_offset + max).min(self.all_results.len());
        let visible: Vec<DisplayablePath> = self.all_results[self.scroll_offset..end].to_vec();
        self.results = SelectableVec::new(visible);
        if !self.results.is_empty() {
            self.results.set_selected_index(cursor_index).ok();
        }
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
        // Filter paths into all_results (no truncation)
        if self.input.is_empty() {
            self.all_results = self.paths.clone();
        } else {
            let query = self.input.to_lowercase();
            self.all_results = self
                .paths
                .iter()
                .filter(|p| p.0.to_string_lossy().to_lowercase().contains(&query))
                .cloned()
                .collect();
        }

        self.scroll_offset = 0;
        self.update_visible_results(0);
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
        if self.results.selected_index() == 0 {
            // At top of visible window — scroll up if possible
            if self.scroll_offset > 0 {
                self.scroll_offset -= 1;
                self.update_visible_results(0);
            }
        } else {
            self.results.select_previous();
        }
    }

    fn select_next(&mut self) {
        let visible_len = self.results.len();
        if visible_len == 0 {
            return;
        }
        if self.results.selected_index() >= visible_len - 1 {
            // At bottom of visible window — scroll down if possible
            if self.scroll_offset + self.config.max_results < self.all_results.len() {
                self.scroll_offset += 1;
                self.update_visible_results(visible_len - 1);
            }
        } else {
            self.results.select_next();
        }
    }

    fn config(&self) -> &SearchSelectConfig {
        &self.config
    }

    fn message(&mut self) -> Option<String> {
        if self.paths.is_empty() {
            Some(String::from("No files found (is 'fd' installed?)"))
        } else if !self.query().is_empty() && self.all_results.is_empty() {
            Some(String::from("No matching entries found."))
        } else {
            None
        }
    }
}
