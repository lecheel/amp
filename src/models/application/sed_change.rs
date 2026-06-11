use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct SedChange {
    pub file: PathBuf,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: usize,
    pub column: usize,
    pub old_text: String,
    pub new_text: String,
    pub confirmed: bool,
    pub context_line: String,
    pub word_boundary: bool,
}
