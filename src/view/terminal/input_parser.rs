use crate::input::Key;
use crate::models::application::Event;

const BRACKETED_PASTE_START: &[u8] = b"\x1b[200~";
const BRACKETED_PASTE_END: &[u8] = b"\x1b[201~";

pub struct InputParser {
    data: Vec<u8>,
    offset: usize,
    // When Some, we're collecting a bracketed paste.
    paste_buffer: Option<Vec<u8>>,
}

impl InputParser {
    pub fn new() -> InputParser {
        InputParser {
            data: Vec::new(),
            offset: 0,
            paste_buffer: None,
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.data.extend_from_slice(data);
    }

    fn continue_paste(&mut self) -> Option<Event> {
        let slice = &self.data[self.offset..];

        // Look for the end sequence
        if let Some(end_pos) = find_subsequence(slice, BRACKETED_PASTE_END) {
            // Found the end — extract all content before it
            let content = &slice[..end_pos];
            self.offset += end_pos + BRACKETED_PASTE_END.len();

            let mut paste_content = self.paste_buffer.take().unwrap();
            paste_content.extend_from_slice(content);

            // Clean up consumed data if we've processed everything
            if self.offset >= self.data.len() {
                self.data.clear();
                self.offset = 0;
            }

            // Normalize line endings
            let text = String::from_utf8_lossy(&paste_content)
                .replace("\r\n", "\n")
                .replace('\r', "\n");

            return Some(Event::Paste(text));
        }

        // End not found yet. Check if the tail of the data could be
        // a partial end sequence so we don't consume it prematurely.
        let keep = keep_partial_match(slice, BRACKETED_PASTE_END);

        // Buffer everything except the potential partial match
        let buffer = self.paste_buffer.as_mut().unwrap();
        buffer.extend_from_slice(&slice[..slice.len().saturating_sub(keep)]);

        if keep > 0 {
            // Move the partial match to the start of data for the next read
            let partial_start = self.data.len() - keep;
            let remaining = self.data[partial_start..].to_vec();
            self.data = remaining;
            self.offset = 0;
        } else {
            self.data.clear();
            self.offset = 0;
        }

        None
    }
}

impl Iterator for InputParser {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        // Continue paste if we're in the middle of one
        if self.paste_buffer.is_some() {
            return self.continue_paste();
        }

        if self.offset >= self.data.len() {
            self.data.clear();
            self.offset = 0;
            return None;
        }

        let slice = &self.data[self.offset..];

        // Check for bracketed paste start sequence (before Alt matching)
        if starts_with(slice, BRACKETED_PASTE_START) {
            self.offset += BRACKETED_PASTE_START.len();
            self.paste_buffer = Some(Vec::new());
            return self.continue_paste();
        }

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
            // F1-F12
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
            // Bare Escape
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

fn starts_with(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.len() >= needle.len() && &haystack[..needle.len()] == needle
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Returns the number of bytes at the end of `data` that could be
/// the start of `pattern`. This prevents consuming bytes that are
/// part of a split escape sequence.
fn keep_partial_match(data: &[u8], pattern: &[u8]) -> usize {
    if data.is_empty() || pattern.len() <= 1 {
        return 0;
    }

    let max_check = data.len().min(pattern.len() - 1);
    for len in (1..=max_check).rev() {
        let data_suffix = &data[data.len() - len..];
        let pattern_prefix = &pattern[..len];
        if data_suffix == pattern_prefix {
            return len;
        }
    }
    0
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

    #[test]
    fn parses_bracketed_paste() {
        let mut parser = InputParser::new();
        let paste_data = b"\x1b[200~hello world\x1b[201~";
        parser.feed(paste_data);
        assert_eq!(parser.next(), Some(Event::Paste("hello world".to_string())));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn parses_bracketed_paste_with_newlines() {
        let mut parser = InputParser::new();
        let paste_data = b"\x1b[200~line1\r\nline2\r\nline3\x1b[201~";
        parser.feed(paste_data);
        assert_eq!(
            parser.next(),
            Some(Event::Paste("line1\nline2\nline3".to_string()))
        );
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn parses_bracketed_paste_followed_by_key() {
        let mut parser = InputParser::new();
        let paste_data = b"\x1b[200~paste\x1b[201~a";
        parser.feed(paste_data);
        assert_eq!(parser.next(), Some(Event::Paste("paste".to_string())));
        assert_eq!(parser.next(), Some(Event::Key(Key::Char('a'))));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn handles_split_bracketed_paste() {
        let mut parser = InputParser::new();

        // First read: start sequence and partial content
        parser.feed(b"\x1b[200~hel");
        assert_eq!(parser.next(), None);

        // Second read: rest of content and end sequence
        parser.feed(b"lo\x1b[201~");
        assert_eq!(parser.next(), Some(Event::Paste("hello".to_string())));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn handles_partial_end_sequence_at_buffer_boundary() {
        let mut parser = InputParser::new();

        // First read: content ending with partial end sequence
        parser.feed(b"\x1b[200~content\x1b[20");
        assert_eq!(parser.next(), None);

        // Second read: rest of end sequence
        parser.feed(b"1~");
        assert_eq!(parser.next(), Some(Event::Paste("content".to_string())));
        assert_eq!(parser.next(), None);
    }

    #[test]
    fn normal_keys_after_paste() {
        let mut parser = InputParser::new();
        parser.feed(b"\x1b[200~abc\x1b[201~\x1b[A");
        assert_eq!(parser.next(), Some(Event::Paste("abc".to_string())));
        assert_eq!(parser.next(), Some(Event::Key(Key::Up)));
        assert_eq!(parser.next(), None);
    }
}
