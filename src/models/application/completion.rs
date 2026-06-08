use std::collections::BTreeSet;

#[derive(Clone, Debug)]
pub struct CompletionEntry {
    pub display: String,
    pub value: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CompletionOrigin {
    BufferWords,
    ExInput,
}

#[derive(Clone, Debug)]
pub struct CompletionState {
    pub entries: Vec<CompletionEntry>,
    pub selected_index: usize,
    pub prefix: String,
    pub origin: CompletionOrigin,
}

impl CompletionState {
    pub fn select_next(&mut self) {
        if !self.entries.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.entries.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.entries.is_empty() {
            if self.selected_index > 0 {
                self.selected_index -= 1;
            } else {
                self.selected_index = self.entries.len() - 1;
            }
        }
    }

    pub fn selection(&self) -> Option<&CompletionEntry> {
        self.entries.get(self.selected_index)
    }

    /// Scan `buffer_data` for words that extend `prefix`.
    /// Case-sensitive matching; returns at most 100 entries.
    /// Requires a minimum prefix length of 3 to avoid flooding candidates.
    pub fn from_buffer_words(
        buffer_data: &str,
        prefix: &str,
        origin: CompletionOrigin,
    ) -> Option<Self> {
        if prefix.chars().count() < 3 {
            return None;
        }

        let mut word_set = BTreeSet::new();

        for line in buffer_data.lines() {
            for word in line.split(|c: char| !c.is_alphanumeric() && c != '_') {
                if word.len() > prefix.len() && word.starts_with(prefix) {
                    word_set.insert(word.to_string());
                }
            }
        }

        let entries: Vec<CompletionEntry> = word_set
            .into_iter()
            .take(100)
            .map(|word| CompletionEntry {
                display: word.clone(),
                value: word,
            })
            .collect();

        if entries.is_empty() {
            None
        } else {
            Some(Self {
                entries,
                selected_index: 0,
                prefix: prefix.to_string(),
                origin,
            })
        }
    }
}
