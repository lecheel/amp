#[derive(Clone, Debug)]
pub enum RepeatableAction {
    DeleteCurrentLine,
    DeleteToken,
    DeleteRestOfLine,
    DeleteAroundFunction,
    ChangeCurrentLine,
    ChangeToken,
    ChangeRestOfLine,
    Paste,
    PasteAbove,
    IndentLine,
    OutdentLine,
    ToggleLineComment,
    MergeNextLine,
    InsertModeEntry, // Plain insert mode (i, a, o, etc.)
}
