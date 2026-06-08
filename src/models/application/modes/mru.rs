use crate::models::application::modes::open::DisplayablePath;
use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use std::fmt;
use std::slice::Iter;

pub struct MRUMode {
    insert: bool,
    input: String,
    paths: Vec<DisplayablePath>,
    results: SelectableVec<DisplayablePath>,
    config: SearchSelectConfig,
}

impl MRUMode {
    pub fn new(config: SearchSelectConfig) -> MRUMode {
        MRUMode {
            insert: true,
            input: String::new(),
            paths: Vec::new(),
            results: SelectableVec::new(Vec::new()),
            config,
        }
    }

    pub fn reset(&mut self, paths: Vec<DisplayablePath>, config: SearchSelectConfig) {
        self.input.clear();
        self.insert = true;
        self.paths = paths;
        self.results = SelectableVec::new(self.paths.clone());
        self.config = config;
    }
}

impl fmt::Display for MRUMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MRU")
    }
}

impl SearchSelectMode for MRUMode {
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
}
