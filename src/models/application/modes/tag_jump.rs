use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use fragment;
use fragment::matching::AsStr;
use std::fmt;
use std::path::PathBuf;
use std::slice::Iter;

#[derive(Clone, Debug)]
pub struct TagEntry {
    pub name: String,
    pub file: PathBuf,
    pub line: usize,
    pub kind: Option<String>,
}

impl fmt::Display for TagEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind_str = self
            .kind
            .as_deref()
            .map(|k| format!(" [{}]", k))
            .unwrap_or_default();
        write!(
            f,
            "{}\t{}:{}{}",
            self.name,
            self.file.display(),
            self.line,
            kind_str
        )
    }
}

impl AsStr for TagEntry {
    fn as_str(&self) -> &str {
        &self.name
    }
}

pub struct TagJumpMode {
    insert: bool,
    input: String,
    tags: Vec<TagEntry>,
    all_results: Vec<TagEntry>,
    scroll_offset: usize,
    results: SelectableVec<TagEntry>,
    config: SearchSelectConfig,
}

impl TagJumpMode {
    pub fn new(config: SearchSelectConfig) -> TagJumpMode {
        TagJumpMode {
            insert: true,
            input: String::new(),
            tags: Vec::new(),
            all_results: Vec::new(),
            scroll_offset: 0,
            results: SelectableVec::new(Vec::new()),
            config,
        }
    }

    pub fn reset(&mut self, tags: Vec<TagEntry>, config: SearchSelectConfig) {
        self.insert = true;
        self.input.clear();
        self.tags = tags;
        self.all_results = Vec::new();
        self.scroll_offset = 0;
        self.results = SelectableVec::new(Vec::new());
        self.config = config;
    }

    fn update_visible_results(&mut self, cursor_index: usize) {
        let max = self.config.max_results;
        let end = (self.scroll_offset + max).min(self.all_results.len());
        let visible: Vec<TagEntry> = self.all_results[self.scroll_offset..end].to_vec();
        self.results = SelectableVec::new(visible);
        if !self.results.is_empty() {
            self.results.set_selected_index(cursor_index).ok();
        }
    }
}

impl fmt::Display for TagJumpMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TAG")
    }
}

impl SearchSelectMode for TagJumpMode {
    type Item = TagEntry;

    fn search(&mut self) {
        if self.input.is_empty() {
            self.all_results = self.tags.clone();
        } else {
            self.all_results = fragment::matching::find(&self.input, &self.tags, usize::MAX)
                .into_iter()
                .map(|i| i.clone())
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

    fn results(&self) -> Iter<'_, TagEntry> {
        self.results.iter()
    }

    fn selection(&self) -> Option<&TagEntry> {
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
