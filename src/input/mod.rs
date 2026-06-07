pub use self::key_map::KeyMap;

pub mod key_map;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Key {
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Insert,
    Esc,
    Tab,
    Enter,
    AnyChar,
    Char(char),
    Ctrl(char),
    Alt(char),
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}
