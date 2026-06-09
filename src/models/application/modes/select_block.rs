use scribe::buffer::{Position, Range};
use std::fmt;

pub struct SelectBlockMode {
    pub anchor: Position,
}

impl SelectBlockMode {
    pub fn new(anchor: Position) -> SelectBlockMode {
        SelectBlockMode { anchor }
    }

    pub fn reset(&mut self, anchor: Position) {
        self.anchor = anchor;
    }

    pub fn to_ranges(&self, cursor: &Position) -> Vec<Range> {
        let min_line = self.anchor.line.min(cursor.line);
        let max_line = self.anchor.line.max(cursor.line);
        let min_offset = self.anchor.offset.min(cursor.offset);
        // Always +1: Range is exclusive on the right, so we need max+1
        // to include the character at max_offset on every line uniformly.
        let end_offset = self.anchor.offset.max(cursor.offset) + 1;

        (min_line..=max_line)
            .map(|line| {
                Range::new(
                    Position {
                        line,
                        offset: min_offset,
                    },
                    Position {
                        line,
                        offset: end_offset,
                    },
                )
            })
            .collect()
    }
}

impl fmt::Display for SelectBlockMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "BLOCK")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockInsertType {
    Insert,
    Append,
}

pub struct BlockInsertMode {
    pub start_line: usize,
    pub end_line: usize,
    pub left_column: usize,
    pub right_column: usize,
    pub insert_column: usize,
    pub insert_type: BlockInsertType,
    pub typed_text: String,
}

impl BlockInsertMode {
    pub fn new() -> BlockInsertMode {
        BlockInsertMode {
            start_line: 0,
            end_line: 0,
            left_column: 0,
            right_column: 0,
            insert_column: 0,
            insert_type: BlockInsertType::Insert,
            typed_text: String::new(),
        }
    }

    pub fn reset(&mut self, anchor: Position, cursor: Position, insert_type: BlockInsertType) {
        self.start_line = anchor.line.min(cursor.line);
        self.end_line = anchor.line.max(cursor.line);
        self.left_column = anchor.offset.min(cursor.offset);
        self.right_column = anchor.offset.max(cursor.offset);
        self.insert_column = match insert_type {
            BlockInsertType::Insert => self.left_column,
            BlockInsertType::Append => self.right_column + 1,
        };
        self.insert_type = insert_type;
        self.typed_text.clear();
    }

    pub fn to_ranges(&self) -> Vec<Range> {
        // Always +1 for the same reason: inclusive right edge.
        let end_offset = self.right_column + 1;

        (self.start_line..=self.end_line)
            .map(|line| {
                Range::new(
                    Position {
                        line,
                        offset: self.left_column,
                    },
                    Position {
                        line,
                        offset: end_offset,
                    },
                )
            })
            .collect()
    }
}

impl fmt::Display for BlockInsertMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.insert_type {
            BlockInsertType::Insert => write!(f, "BLOCK INSERT"),
            BlockInsertType::Append => write!(f, "BLOCK APPEND"),
        }
    }
}
