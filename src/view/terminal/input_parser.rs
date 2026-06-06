use crate::input::Key;
use crate::models::application::Event;

pub struct InputParser {
    data: Vec<u8>,
    offset: usize,
}

impl InputParser {
    pub fn new() -> InputParser {
        InputParser {
            data: Vec::new(),
            offset: 0,
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }
}

impl Iterator for InputParser {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset >= self.data.len() {
            self.data.clear();
            self.offset = 0;
            return None;
        }

        let slice = &self.data[self.offset..];

        let (key, consumed) = match slice {
            [0x1B, b'[', b'A', ..] => (Key::Up, 3),
            [0x1B, b'[', b'B', ..] => (Key::Down, 3),
            [0x1B, b'[', b'C', ..] => (Key::Right, 3),
            [0x1B, b'[', b'D', ..] => (Key::Left, 3),
            [0x1B, b'[', b'H', ..] => (Key::Home, 3),
            [0x1B, b'[', b'F', ..] => (Key::End, 3),
            [0x1B, b'[', b'2', b'~', ..] => (Key::Insert, 4),
            [0x1B, b'[', b'3', b'~', ..] => (Key::Delete, 4),
            [0x1B, b'[', b'5', b'~', ..] => (Key::PageUp, 4),
            [0x1B, b'[', b'6', b'~', ..] => (Key::PageDown, 4),
            // F1-F12 (CSI sequences: ESC [ 11~ through ESC [ 24~)
            [0x1B, b'[', b'1', b'1', b'~', ..] => (Key::F1, 5),
            [0x1B, b'[', b'1', b'2', b'~', ..] => (Key::F2, 5),
            [0x1B, b'[', b'1', b'3', b'~', ..] => (Key::F3, 5),
            [0x1B, b'[', b'1', b'4', b'~', ..] => (Key::F4, 5),
            [0x1B, b'[', b'1', b'5', b'~', ..] => (Key::F5, 5),
            [0x1B, b'[', b'1', b'7', b'~', ..] => (Key::F6, 5),
            [0x1B, b'[', b'1', b'8', b'~', ..] => (Key::F7, 5),
            [0x1B, b'[', b'1', b'9', b'~', ..] => (Key::F8, 5),
            [0x1B, b'[', b'2', b'0', b'~', ..] => (Key::F9, 5),
            [0x1B, b'[', b'2', b'1', b'~', ..] => (Key::F10, 5),
            [0x1B, b'[', b'2', b'3', b'~', ..] => (Key::F11, 5),
            [0x1B, b'[', b'2', b'4', b'~', ..] => (Key::F12, 5),
            // Alt + printable ASCII character
            [0x1B, b @ 0x20..=0x7E, ..] => (Key::Alt(*b as char), 2),
            // Alt + UTF-8 character
            [0x1B, b @ 0x80..=0xFF, ..] => {
                let (key, utf8_len) = utf8_char(&slice[1..], *b)?;
                match key {
                    Key::Char(c) => (Key::Alt(c), 1 + utf8_len),
                    _ => return None,
                }
            }
            // Bare Escape (no following character in this batch)
            [0x1B, ..] => (Key::Esc, 1),
            [0x1C, ..] => (Key::Ctrl('\\'), 1),
            [0x1D, ..] => (Key::Ctrl(']'), 1),
            [0x1E, ..] => (Key::Ctrl('^'), 1),
            [0x1F, ..] => (Key::Ctrl('_'), 1),
            [0x7F, ..] | [0x08, ..] => (Key::Backspace, 1),
            [0x0A, ..] | [0x0D, ..] => (Key::Enter, 1),
            [0x09, ..] => (Key::Tab, 1),
            [b @ 0x01..=0x1A, ..] => (Key::Ctrl((b + b'a' - 1) as char), 1),
            [b @ 0x20..=0x7E, ..] => (Key::Char(*b as char), 1),
            [b @ 0x80..=0xFF, ..] => utf8_char(slice, *b)?,
            _ => return None,
        };

        self.offset += consumed;

        Some(Event::Key(key))
    }
}

fn utf8_char(slice: &[u8], first_byte: u8) -> Option<(Key, usize)> {
    let len = match first_byte {
        0xC2..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xF4 => 4,
        _ => return None,
    };

    let data = slice.get(..len)?;
    let character = std::str::from_utf8(data).ok()?.chars().next()?;

    Some((Key::Char(character), len))
}

#[cfg(test)]
mod tests {
    use super::InputParser;
    use crate::input::Key;
    use crate::models::application::Event;

    #[test]
    fn parses_utf8_character_input() {
        let mut parser = InputParser::new();

        parser.feed("é".as_bytes());

        assert_eq!(parser.next(), Some(Event::Key(Key::Char('é'))));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn waits_for_complete_utf8_character_input() {
        let mut parser = InputParser::new();

        parser.feed(&[0xC3]);
        assert_eq!(parser.next(), None);

        parser.feed(&[0xA9]);

        assert_eq!(parser.next(), Some(Event::Key(Key::Char('é'))));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn parses_utf8_character_followed_by_ascii_input() {
        let mut parser = InputParser::new();

        parser.feed("éa".as_bytes());

        assert_eq!(parser.next(), Some(Event::Key(Key::Char('é'))));
        assert_eq!(parser.next(), Some(Event::Key(Key::Char('a'))));
        assert_eq!(parser.next(), None);
    }
}
