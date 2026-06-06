mod command;
mod confirm;
pub mod jump;
mod line_jump;
pub mod open;
mod path;
mod search;
mod search_select;
mod select;
mod select_line;
mod symbol_jump;
mod syntax;
mod theme;

use std::fmt;

pub enum Mode {
    Command(CommandMode),
    Confirm(ConfirmMode),
    Exit,
    Insert,
    Jump(JumpMode),
    LineJump(LineJumpMode),
    Normal,
    Open(OpenMode),
    Paste,
    Path(PathMode),
    PendingChange(PendingChangeMode),
    PendingDelete(PendingDeleteMode),
    PendingLeftBracket(PendingLeftBracketMode),
    PendingRightBracket(PendingRightBracketMode),
    PendingYank(PendingYankMode),
    Search(SearchMode),
    Select(SelectMode),
    SelectLine(SelectLineMode),
    SymbolJump(SymbolJumpMode),
    Syntax(SyntaxMode),
    Theme(ThemeMode),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModeKey {
    Command,
    Confirm,
    Exit,
    Insert,
    Jump,
    LineJump,
    Normal,
    Open,
    Paste,
    Path,
    PendingChange,
    PendingDelete,
    PendingLeftBracket,
    PendingRightBracket,
    PendingYank,
    Search,
    Select,
    SelectLine,
    SymbolJump,
    Syntax,
    Theme,
}

pub use self::command::CommandMode;
pub use self::confirm::ConfirmMode;
pub use self::jump::JumpMode;
pub use self::line_jump::LineJumpMode;
pub use self::open::OpenMode;
pub use self::path::PathMode;
pub use self::search::SearchMode;
pub use self::search_select::{PopSearchToken, SearchSelectConfig, SearchSelectMode};
pub use self::select::SelectMode;
pub use self::select_line::SelectLineMode;
pub use self::symbol_jump::SymbolJumpMode;
pub use self::syntax::SyntaxMode;
pub use self::theme::ThemeMode;

// Pending mode types

#[derive(Default)]
pub struct PendingChangeMode;

impl PendingChangeMode {
    pub fn new() -> PendingChangeMode {
        PendingChangeMode::default()
    }
}

impl fmt::Display for PendingChangeMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CHANGE")
    }
}

#[derive(Default)]
pub struct PendingDeleteMode;

impl PendingDeleteMode {
    pub fn new() -> PendingDeleteMode {
        PendingDeleteMode::default()
    }
}

impl fmt::Display for PendingDeleteMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "DELETE")
    }
}

#[derive(Default)]
pub struct PendingLeftBracketMode;

impl PendingLeftBracketMode {
    pub fn new() -> PendingLeftBracketMode {
        PendingLeftBracketMode::default()
    }
}

impl fmt::Display for PendingLeftBracketMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "[")
    }
}

#[derive(Default)]
pub struct PendingRightBracketMode;

impl PendingRightBracketMode {
    pub fn new() -> PendingRightBracketMode {
        PendingRightBracketMode::default()
    }
}

impl fmt::Display for PendingRightBracketMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "]")
    }
}

#[derive(Default)]
pub struct PendingYankMode;

impl PendingYankMode {
    pub fn new() -> PendingYankMode {
        PendingYankMode::default()
    }
}

impl fmt::Display for PendingYankMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "YANK")
    }
}
