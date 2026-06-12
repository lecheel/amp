use crate::view::{Colors, Style};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Left,
    Expand,
    Right,
}

impl Default for Alignment {
    fn default() -> Self {
        Alignment::Left
    }
}

#[derive(Debug, Clone)]
pub struct StatusLineData {
    pub content: String,
    pub style: Style,
    pub colors: Colors,
    pub alignment: Alignment,
}
