mod command;
mod confirm;
mod ex;
pub mod fd;
pub mod file_picker;
pub mod jump;
mod line_jump;
mod mru;
pub mod open;
mod path;
mod search;
mod search_select;
mod select;
mod select_line;
mod symbol_jump;
mod syntax;
mod theme;

pub use self::ex::ExMode;
pub use self::fd::FdMode;
pub use self::file_picker::FilePickerMode;
pub use self::mru::MRUMode;
use std::fmt;

pub enum Mode {
    Command(CommandMode),
    Confirm(ConfirmMode),
    Exit,
    Fd(FdMode),
    FilePicker(FilePickerMode),
    Insert,
    Jump(JumpMode),
    LineJump(LineJumpMode),
    Normal,
    Ex(ExMode),
    Open(OpenMode),
    Paste,
    Path(PathMode),
    PendingChange(PendingChangeMode),
    PendingDelete(PendingDeleteMode),
    PendingLeftBracket(PendingLeftBracketMode),
    PendingRightBracket(PendingRightBracketMode),
    PendingYank(PendingYankMode),
    PendingLeader(PendingLeaderMode),
    Search(SearchMode),
    Select(SelectMode),
    SelectLine(SelectLineMode),
    SymbolJump(SymbolJumpMode),
    Syntax(SyntaxMode),
    Theme(ThemeMode),
    MRU(MRUMode),
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ModeKey {
    Command,
    Confirm,
    Exit,
    Fd,
    FilePicker,
    Insert,
    Jump,
    LineJump,
    Normal,
    Open,
    Ex,
    Paste,
    Path,
    PendingChange,
    PendingDelete,
    PendingLeftBracket,
    PendingRightBracket,
    PendingYank,
    Leader,
    Search,
    Select,
    SelectLine,
    SymbolJump,
    Syntax,
    Theme,
    MRU,
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
use crate::input::Key;

// Pending mode types

#[derive(Default)]
pub struct PendingLeaderMode {
    pub keys: Vec<Key>,
}

impl PendingLeaderMode {
    pub fn new() -> Self {
        PendingLeaderMode { keys: Vec::new() }
    }
    pub fn reset(&mut self) {
        self.keys.clear();
    }
}

impl fmt::Display for PendingLeaderMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LEADER")
    }
}

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
