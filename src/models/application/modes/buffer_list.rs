use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use std::fmt;
use std::path::PathBuf;
use std::slice::Iter;

#[derive(Clone, Debug)]
pub struct BufferEntry {
    pub path: Option<PathBuf>,
    pub buffer_id: Option<usize>,
    pub modified: bool,
}

impl fmt::Display for BufferEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let path_str = self
            .path
            .as_ref()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_else(|| "[No Name]".to_string());
        let modified = if self.modified { " [+]" } else { "" };
        write!(f, "{}{}", path_str, modified)
    }
}

pub struct BufferListMode {
    pub insert: bool,
    input: String,
    entries: Vec<BufferEntry>,
    all_results: Vec<BufferEntry>,
    results: SelectableVec<BufferEntry>,
    config: SearchSelectConfig,
    pub scroll_offset: usize,
}

impl BufferListMode {
    pub fn new(config: SearchSelectConfig) -> BufferListMode {
        BufferListMode {
            insert: true,
            input: String::new(),
            entries: Vec::new(),
            all_results: Vec::new(),
            results: SelectableVec::new(Vec::new()),
            scroll_offset: 0,
            config,
        }
    }

    pub fn reset(&mut self, entries: Vec<BufferEntry>, config: SearchSelectConfig) {
        self.input.clear();
        self.insert = true;
        self.config = config;
        self.scroll_offset = 0;
        self.entries = entries;
        self.search();
    }
}

impl fmt::Display for BufferListMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BUFFERS")
    }
}

impl SearchSelectMode for BufferListMode {
    type Item = BufferEntry;

    fn search(&mut self) {
        if self.input.is_empty() {
            self.all_results = self.entries.clone();
        } else {
            let query = self.input.to_lowercase();
            self.all_results = self
                .entries
                .iter()
                .filter(|e| e.to_string().to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
        self.results = SelectableVec::new(self.all_results.clone());
        if !self.results.is_empty() {
            self.results.set_selected_index(0).ok();
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

    fn results(&self) -> Iter<'_, BufferEntry> {
        self.results.iter()
    }

    fn selection(&self) -> Option<&BufferEntry> {
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
        if self.entries.is_empty() {
            Some(String::from("No buffers open."))
        } else if !self.query().is_empty() && self.all_results.is_empty() {
            Some(String::from("No matching entries found."))
        } else {
            None
        }
    }
}
