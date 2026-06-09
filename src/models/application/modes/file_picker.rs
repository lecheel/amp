use crate::models::application::modes::open::DisplayablePath;
use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use std::fmt;
use std::fs;
use std::path::PathBuf;
use std::slice::Iter;

pub struct FilePickerMode {
    pub insert: bool,
    input: String,
    pub current_dir: PathBuf,
    entries: Vec<DisplayablePath>,
    all_results: Vec<DisplayablePath>,
    results: SelectableVec<DisplayablePath>,
    config: SearchSelectConfig,
}

impl FilePickerMode {
    pub fn new(config: SearchSelectConfig) -> FilePickerMode {
        FilePickerMode {
            insert: true,
            input: String::new(),
            current_dir: PathBuf::new(),
            entries: Vec::new(),
            all_results: Vec::new(),
            results: SelectableVec::new(Vec::new()),
            config,
        }
    }

    pub fn reset(&mut self, workspace_path: &PathBuf, config: SearchSelectConfig) {
        self.current_dir = workspace_path.clone();
        self.input.clear();
        self.insert = true;
        self.config = config;
        self.reload();
    }

    pub fn reload(&mut self) {
        self.entries = fs::read_dir(&self.current_dir)
            .map(|entries| {
                let mut paths: Vec<DisplayablePath> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| DisplayablePath(e.path()))
                    .collect();
                // Sort directories first, then alphabetically
                paths.sort_by(|a, b| {
                    let a_is_dir = a.0.is_dir();
                    let b_is_dir = b.0.is_dir();
                    b_is_dir
                        .cmp(&a_is_dir)
                        .then(a.0.file_name().cmp(&b.0.file_name()))
                });
                paths
            })
            .unwrap_or_default();

        self.search();
    }

    pub fn navigate_up(&mut self) {
        if self.current_dir.pop() {
            self.input.clear();
            self.reload();
        }
    }
}

impl fmt::Display for FilePickerMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PICKER")
    }
}

impl SearchSelectMode for FilePickerMode {
    type Item = DisplayablePath;

    fn search(&mut self) {
        if self.input.is_empty() {
            self.all_results = self.entries.clone();
        } else {
            let query = self.input.to_lowercase();
            self.all_results = self
                .entries
                .iter()
                .filter(|p| {
                    p.0.file_name()
                        .map(|f| f.to_string_lossy().to_lowercase().contains(&query))
                        .unwrap_or(false)
                })
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
