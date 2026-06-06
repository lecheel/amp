use std::fmt;

#[derive(Default)]
pub struct ExMode {
    pub input: String,
    pub history: Vec<String>,
    pub history_index: usize,
}

impl ExMode {
    pub fn new() -> ExMode {
        ExMode::default()
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.history_index = self.history.len();
    }
}

impl fmt::Display for ExMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CMD")
    }
}
