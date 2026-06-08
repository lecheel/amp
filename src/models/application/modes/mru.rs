use crate::models::application::modes::open::DisplayablePath;
use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use std::fmt;
use std::slice::Iter;

pub struct MRUMode {
    insert: bool,
    input: String,
    paths: Vec<DisplayablePath>,
    all_results: Vec<DisplayablePath>,
    scroll_offset: usize,
    results: SelectableVec<DisplayablePath>,
    config: SearchSelectConfig,
}

impl MRUMode {
    pub fn new(config: SearchSelectConfig) -> MRUMode {
        MRUMode {
            insert: true,
            input: String::new(),
            paths: Vec::new(),
            all_results: Vec::new(),
            scroll_offset: 0,
            results: SelectableVec::new(Vec::new()),
            config,
        }
    }

    pub fn reset(&mut self, paths: Vec<DisplayablePath>, config: SearchSelectConfig) {
        self.input.clear();
        self.insert = true;
        self.paths = paths;
        self.all_results = self.paths.clone();
        self.scroll_offset = 0;
        self.config = config;
        self.update_visible_results(0);
    }

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

impl fmt::Display for MRUMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "MRU")
    }
}

impl SearchSelectMode for MRUMode {
    type Item = DisplayablePath;

    fn search(&mut self) {
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
}
