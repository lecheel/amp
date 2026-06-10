use crate::errors::*;
use crate::input::Key;
use crate::models::application::CompletionState;
use crate::view::buffer::{BufferRenderer, LexemeMapper};
use crate::view::color::{ColorMap, Colors};
use crate::view::style::Style;
use crate::view::terminal::{Cell, CursorType, TerminalBuffer};
use crate::view::StatusLineData;
use crate::view::View;
use log::{debug, trace};
use scribe::buffer::{Buffer, Position, Range};
use scribe::util::LineIterator;
use std::borrow::Cow;
use syntect::highlighting::Theme;
use syntect::parsing::SyntaxSet;
use unicode_segmentation::UnicodeSegmentation;

/// The `Presenter` type forms the main view API for mode-specific presenters.
/// It provides the ability to read view dimensions, draw individual character
/// "cells", and render higher-level components like buffers. Writes are
/// buffered and flushed to the terminal with the `present` method.
pub struct Presenter<'p> {
    cursor_position: Option<Position>,
    terminal_buffer: TerminalBuffer<'p>,
    theme: Theme,
    pub view: &'p mut View,
    pub overlay: bool,
}

impl<'p> Presenter<'p> {
    pub fn new(view: &mut View) -> Result<Presenter<'_>> {
        debug!("establishing theme");

        let theme = {
            let preferences = view.preferences.borrow();
            let theme_name = preferences.theme();
            let theme = view
                .theme_set
                .themes
                .get(theme_name)
                .or_else(|| {
                    let default_theme_name = preferences.default_theme();
                    debug!(
                        "theme \"{}\" not found; falling back to \"{}\"",
                        theme_name, default_theme_name
                    );
                    view.theme_set.themes.get(default_theme_name)
                })
                .ok_or_else(|| anyhow!("Couldn't find \"{theme_name}\" theme"))?;
            theme.clone()
        };

        Ok(Presenter {
            cursor_position: None,
            terminal_buffer: TerminalBuffer::new(view.terminal.width(), view.terminal.height()),
            theme,
            view,
            overlay: false,
        })
    }

    pub fn width(&self) -> usize {
        self.view.terminal.width()
    }

    pub fn height(&self) -> usize {
        self.view.terminal.height()
    }

    pub fn clear(&mut self) {
        self.terminal_buffer.clear()
    }

    pub fn set_cursor(&mut self, position: Option<Position>) {
        self.cursor_position = position;
    }

    pub fn set_cursor_type(&mut self, cursor_type: CursorType) {
        self.view.terminal.set_cursor_type(cursor_type);
    }

    pub fn present(&mut self) -> Result<()> {
        debug!("rendering terminal buffer to terminal");
        let transparent_background = self.view.preferences.borrow().transparent_background();

        for (position, cell) in self.terminal_buffer.iter() {
            // In overlay mode, skip default cells to preserve the underlying render
            if self.overlay
                && cell.content == " "
                && matches!(cell.colors, Colors::Default)
                && matches!(cell.style, Style::Default)
            {
                continue;
            }

            self.view.terminal.print(
                &position,
                cell.style,
                self.theme.map_colors(cell.colors, transparent_background),
                &cell.content,
            )?;
        }

        debug!("rendering terminal cursor");

        self.view.terminal.set_cursor(self.cursor_position);

        debug!("flushing terminal");

        self.view.terminal.present();

        Ok(())
    }

    pub fn print_buffer(
        &mut self,
        buffer: &Buffer,
        buffer_data: &'p str,
        syntax_set: &'p SyntaxSet,
        highlights: Option<&[Range]>,
        lexeme_mapper: Option<&'p mut dyn LexemeMapper>,
    ) -> Result<()> {
        let scroll_offset = self.view.get_region(buffer)?.line_offset();
        let lines = LineIterator::new(buffer_data);
        let gutter_statuses = self.view.gutter_statuses.clone();

        debug!("rendering buffer");

        self.cursor_position = BufferRenderer::new(
            buffer,
            highlights,
            scroll_offset,
            &**self.view.terminal,
            &self.theme,
            &self.view.preferences.borrow(),
            self.view.get_render_cache(buffer)?,
            syntax_set,
            &mut self.terminal_buffer,
            gutter_statuses,
        )
        .render(lines, lexeme_mapper)?;

        Ok(())
    }

    pub fn print_status_line(&mut self, entries: &[StatusLineData]) {
        let line = self.view.terminal.height() - 1;

        debug!("rendering status line");

        entries
            .iter()
            .enumerate()
            .fold(0, |offset, (index, element)| {
                let content = match entries.len() {
                    // There's only one element; have it fill the line.
                    1 => format!(
                        "{:width$}",
                        element.content,
                        width = self.view.terminal.width(),
                    ),

                    // Expand the last element to fill the remaining width.
                    2 if index == entries.len() - 1 => format!(
                        "{:width$}",
                        element.content,
                        width = self.view.terminal.width().saturating_sub(offset),
                    ),
                    2 => element.content.clone(),

                    _ if index == entries.len() - 2 => {
                        let space = offset + entries[index + 1].content.len();
                        format!(
                            "{:width$}",
                            element.content,
                            width = self.view.terminal.width().saturating_sub(space),
                        )
                    }
                    _ => element.content.clone(),
                };

                // Update the tracked offset.
                let updated_offset = offset + content.len();

                self.print(
                    &Position { line, offset },
                    element.style,
                    element.colors,
                    content,
                );

                updated_offset
            });
    }

    pub fn print_error<I: Into<String>>(&mut self, error: I) {
        debug!("rendering error");

        self.print_status_line(&[StatusLineData {
            content: error.into(),
            style: Style::Bold,
            colors: Colors::Warning,
        }]);
    }

    pub fn print<C>(&mut self, position: &Position, style: Style, colors: Colors, content: C)
    where
        C: Into<Cow<'p, str>>,
    {
        let content = content.into();
        trace!("writing \"{}\" to terminal buffer", content);
        self.terminal_buffer.set_cell(
            *position,
            Cell {
                content: content,
                style,
                colors,
            },
        );
    }

    pub fn print_popup_box(&mut self, title: &str, lines: &[&str], accent: Colors) {
        const MAX_CONTENT_LINES: usize = 8;
        const HORIZONTAL_PAD: usize = 2;
        const BORDER_WIDTH: usize = 1;

        // Don't add title to visible lines anymore, it goes in the border
        let mut visible: Vec<String> = lines
            .iter()
            .take(MAX_CONTENT_LINES)
            .map(|s| s.to_string())
            .collect();
        if lines.len() > MAX_CONTENT_LINES {
            if let Some(last) = visible.last_mut() {
                *last = format!("{}…", last.trim_end());
            }
        }
        if visible.is_empty() && title.is_empty() {
            return;
        }

        let box_w = std::cmp::max((self.width() as f32 * 0.9).ceil() as usize, 20);
        let inner_w = box_w
            .saturating_sub(2 * BORDER_WIDTH)
            .saturating_sub(2 * HORIZONTAL_PAD);

        for line in &mut visible {
            let g: Vec<&str> = line.graphemes(true).collect();
            if g.len() > inner_w {
                *line = format!("{}…", g[..inner_w.saturating_sub(1)].join(""));
            }
        }

        // box_h is now just content + 2 borders
        let box_h = visible.len() + 2;
        let bottom_row = self.height().saturating_sub(2);
        let row0 = bottom_row.saturating_sub(box_h.saturating_sub(1));
        let col0 = self.width().saturating_sub(box_w) / 2;

        let border_fg = crate::view::RGBColor(0x58, 0x5C, 0x6E);
        let dark_bg = crate::view::RGBColor(0x1E, 0x1E, 0x2E);
        let light_fg = crate::view::RGBColor(0xC0, 0xCA, 0xF5);

        let title_fg = match accent {
            Colors::Warning => crate::view::RGBColor(0xF3, 0x8B, 0xA8),
            Colors::Insert => crate::view::RGBColor(0xA6, 0xE3, 0xA1),
            Colors::SelectMode => crate::view::RGBColor(0x89, 0xB4, 0xFA),
            Colors::Custom(fg, _) | Colors::CustomForeground(fg) => fg,
            _ => light_fg,
        };

        let border_colors = Colors::Custom(border_fg, dark_bg);
        let text_colors = Colors::Custom(light_fg, dark_bg);
        let title_colors = Colors::Custom(title_fg, dark_bg);

        // Top border: ╭ Title ───╮
        self.print(
            &Position {
                line: row0,
                offset: col0,
            },
            Style::Default,
            border_colors,
            "╭",
        );

        if title.is_empty() {
            for c in 1..box_w.saturating_sub(1) {
                self.print(
                    &Position {
                        line: row0,
                        offset: col0 + c,
                    },
                    Style::Default,
                    border_colors,
                    "─",
                );
            }
        } else {
            let t_str = format!(" {} ", title);
            let t_len = t_str.graphemes(true).count();
            let max_t_len = box_w.saturating_sub(2);

            if t_len >= max_t_len {
                let truncated: String = t_str.graphemes(true).take(max_t_len).collect();
                for (gi, g) in truncated.graphemes(true).enumerate() {
                    self.print(
                        &Position {
                            line: row0,
                            offset: col0 + 1 + gi,
                        },
                        Style::Bold,
                        title_colors,
                        g.to_string(),
                    );
                }
            } else {
                for (gi, g) in t_str.graphemes(true).enumerate() {
                    self.print(
                        &Position {
                            line: row0,
                            offset: col0 + 1 + gi,
                        },
                        Style::Bold,
                        title_colors,
                        g.to_string(),
                    );
                }
                let dash_count = max_t_len.saturating_sub(t_len);
                for c in 0..dash_count {
                    self.print(
                        &Position {
                            line: row0,
                            offset: col0 + 1 + t_len + c,
                        },
                        Style::Default,
                        border_colors,
                        "─",
                    );
                }
            }
        }
        self.print(
            &Position {
                line: row0,
                offset: col0 + box_w - 1,
            },
            Style::Default,
            border_colors,
            "╮",
        );

        // Content rows: │ text │
        for (i, line) in visible.iter().enumerate() {
            let row = row0 + 1 + i;

            self.print(
                &Position {
                    line: row,
                    offset: col0,
                },
                Style::Default,
                border_colors,
                "│",
            );

            for c in 0..box_w.saturating_sub(2) {
                self.print(
                    &Position {
                        line: row,
                        offset: col0 + 1 + c,
                    },
                    Style::Default,
                    text_colors,
                    " ",
                );
            }

            for (gi, grapheme) in line.graphemes(true).enumerate() {
                if gi >= inner_w {
                    break;
                }
                self.print(
                    &Position {
                        line: row,
                        offset: col0 + BORDER_WIDTH + HORIZONTAL_PAD + gi,
                    },
                    Style::Default,
                    text_colors,
                    grapheme.to_string(),
                );
            }

            self.print(
                &Position {
                    line: row,
                    offset: col0 + box_w - 1,
                },
                Style::Default,
                border_colors,
                "│",
            );
        }

        // Bottom border: ╰───...───╯
        let last_row = row0 + box_h - 1;
        self.print(
            &Position {
                line: last_row,
                offset: col0,
            },
            Style::Default,
            border_colors,
            "╰",
        );
        for c in 1..box_w.saturating_sub(1) {
            self.print(
                &Position {
                    line: last_row,
                    offset: col0 + c,
                },
                Style::Default,
                border_colors,
                "─",
            );
        }
        self.print(
            &Position {
                line: last_row,
                offset: col0 + box_w - 1,
            },
            Style::Default,
            border_colors,
            "╯",
        );
    }

    pub fn print_error_popup(&mut self, title: &str, lines: &[&str]) {
        self.print_popup_box(title, lines, Colors::Warning);
    }

    pub fn print_info_popup(&mut self, title: &str, lines: &[&str]) {
        self.print_popup_box(title, lines, Colors::SelectMode);
    }

    pub fn print_success_popup(&mut self, title: &str, lines: &[&str]) {
        self.print_popup_box(title, lines, Colors::Insert);
    }

    pub fn print_error_popup_from_string(&mut self, title: &str, error: &str) {
        let lines: Vec<&str> = error.lines().collect();
        self.print_error_popup(title, &lines);
    }

    pub fn print_messages_box(&mut self, title: &str, lines: &[&str], _accent: Colors) {
        const MAX_LINES: usize = 8;
        let mut visible: Vec<String> = Vec::new();
        if !title.is_empty() {
            visible.push(title.to_string());
        }
        visible.extend(lines.iter().take(MAX_LINES).map(|s| s.to_string()));
        if lines.len() > MAX_LINES {
            if let Some(last) = visible.last_mut() {
                *last = format!("{}…", last.trim_end());
            }
        }
        if visible.is_empty() {
            return;
        }

        let box_w = std::cmp::max((self.width() as f32 * 0.9).ceil() as usize, 20);
        let inner_w = box_w.saturating_sub(4);

        for line in &mut visible {
            let g: Vec<&str> = line.graphemes(true).collect();
            if g.len() > inner_w {
                *line = format!("{}…", g[..inner_w.saturating_sub(1)].join(""));
            }
        }

        let box_h = visible.len();
        let bottom_row = self.height().saturating_sub(2);
        let row0 = bottom_row.saturating_sub(box_h.saturating_sub(1));
        let col0 = self.width().saturating_sub(box_w) / 2;

        let dark_bg = crate::view::RGBColor(0x1E, 0x1E, 0x2E);
        let light_fg = crate::view::RGBColor(0xC0, 0xCA, 0xF5);
        let dark_colors = Colors::Custom(light_fg, dark_bg);

        for (i, line) in visible.iter().enumerate() {
            let row = row0 + i;

            // Fill background cell-by-cell
            for c in 0..box_w {
                self.print(
                    &Position {
                        line: row,
                        offset: col0 + c,
                    },
                    Style::Default,
                    dark_colors,
                    " ",
                );
            }

            // Write text grapheme-by-grapheme so each column cell is set correctly
            for (gi, grapheme) in line.graphemes(true).enumerate() {
                if gi >= inner_w {
                    break;
                }
                self.print(
                    &Position {
                        line: row,
                        offset: col0 + 2 + gi,
                    },
                    Style::Default,
                    dark_colors,
                    grapheme.to_string(),
                );
            }
        }
    }

    /// Convenience: error message box (white on orange).
    pub fn print_error_box(&mut self, title: &str, lines: &[&str]) {
        self.print_messages_box(title, lines, Colors::Warning);
    }

    /// Convenience: info message box (white on blue).
    pub fn print_info_box(&mut self, title: &str, lines: &[&str]) {
        self.print_messages_box(title, lines, Colors::SelectMode);
    }

    /// Convenience: success message box (white on green).
    pub fn print_success_box(&mut self, title: &str, lines: &[&str]) {
        self.print_messages_box(title, lines, Colors::Insert);
    }

    /// Splits a multi-line error string and displays it in a message box.
    pub fn print_error_box_from_string(&mut self, title: &str, error: &str) {
        let lines: Vec<&str> = error.lines().collect();
        self.print_error_box(title, &lines);
    }

    /// Screen-space cursor position computed by the last `print_buffer` call.
    pub fn cursor_screen_position(&self) -> Option<Position> {
        self.cursor_position
    }

    /// Render the unified completion popup anchored at `anchor` (screen coords).
    /// The popup appears above the anchor when space allows, otherwise below.
    pub fn print_completion_popup(&mut self, completion: &CompletionState, anchor: Position) {
        const COL_WIDTH: usize = 30;
        const COLUMNS: usize = 4;
        const MAX_ROWS: usize = 8;

        let entries = &completion.entries;
        if entries.is_empty() {
            return;
        }

        let row_count = (entries.len() + COLUMNS - 1) / COLUMNS;
        let visible_rows = row_count.min(MAX_ROWS);

        let popup_start_y = if anchor.line >= visible_rows {
            anchor.line - visible_rows
        } else {
            anchor.line + 1
        };

        let popup_bg = crate::view::RGBColor(0x2C, 0x2C, 0x2C);
        let popup_fg = crate::view::RGBColor(0xAB, 0xB2, 0xBF);
        let popup_sel_fg = crate::view::RGBColor(0xFF, 0xFF, 0xFF);
        let popup_sel_bg = crate::view::RGBColor(0x4C, 0x78, 0xCC);

        for row in 0..visible_rows {
            // background for the whole row
            let x_start = anchor.offset;
            let row_width = COLUMNS * COL_WIDTH;
            let clamped = row_width.min(self.width().saturating_sub(x_start));
            self.print(
                &Position {
                    line: popup_start_y + row,
                    offset: x_start,
                },
                Style::Default,
                Colors::Custom(popup_fg, popup_bg),
                " ".repeat(clamped),
            );

            for col in 0..COLUMNS {
                let idx = row * COLUMNS + col;
                if idx >= entries.len() {
                    break;
                }

                let is_selected = idx == completion.selected_index;
                let x = anchor.offset + col * COL_WIDTH;
                let y = popup_start_y + row;

                if y >= self.height() - 1 {
                    break;
                }

                let (fg, bg) = if is_selected {
                    (popup_sel_fg, popup_sel_bg)
                } else {
                    (popup_fg, popup_bg)
                };

                let inner = COL_WIDTH.saturating_sub(1);
                let label =
                    truncate_popup_graphemes(&entries[idx].display, inner.saturating_sub(1));
                let glen = label.graphemes(true).count();
                let pad = inner.saturating_sub(glen);
                let cell = format!(" {}{}", label, " ".repeat(pad));

                self.print(
                    &Position { line: y, offset: x },
                    if is_selected {
                        Style::Bold
                    } else {
                        Style::Default
                    },
                    Colors::Custom(fg, bg),
                    cell,
                );
            }
        }
    }

    pub fn which_key_entries(&self, mode: &str) -> Vec<(String, String)> {
        self.view
            .preferences
            .borrow()
            .keymap()
            .which_key_entries(mode)
    }

    pub fn which_key_leader_entries(&self, keys: &[Key]) -> Vec<(String, String)> {
        self.view
            .preferences
            .borrow()
            .keymap()
            .which_key_leader_entries(keys)
    }

    pub fn print_which_key_popup(&mut self, title: &str, entries: &[(String, String)]) {
        const MIN_WIDTH: usize = 30;
        const MAX_ROWS: usize = 10;
        const HORIZONTAL_PAD: usize = 2;
        const BORDER_WIDTH: usize = 1;
        const GAP: usize = 2;

        if entries.is_empty() {
            return;
        }

        let visible: Vec<&(String, String)> = entries.iter().take(MAX_ROWS).collect();

        // Calculate key column width based on actual entries
        let key_col_width = visible
            .iter()
            .map(|(key, _)| key.graphemes(true).count())
            .max()
            .unwrap_or(3)
            .max(3);

        // Calculate max description width
        let desc_col_width = visible
            .iter()
            .map(|(_, desc)| desc.graphemes(true).count())
            .max()
            .unwrap_or(0);

        // Box dimensions
        let content_width = key_col_width + GAP + desc_col_width;
        let box_w = std::cmp::max(
            MIN_WIDTH,
            content_width + 2 * HORIZONTAL_PAD + 2 * BORDER_WIDTH,
        );
        let box_h = visible.len() + 2; // top + bottom border

        // Position: bottom-right, above status line
        let bottom_row = self.height().saturating_sub(2);
        let row0 = bottom_row.saturating_sub(box_h.saturating_sub(1));
        let col0 = self.width().saturating_sub(box_w);

        // Colors
        let border_fg = crate::view::RGBColor(0x58, 0x5C, 0x6E);
        let dark_bg = crate::view::RGBColor(0x1E, 0x1E, 0x2E);
        let light_fg = crate::view::RGBColor(0xC0, 0xCA, 0xF5);
        let key_fg = crate::view::RGBColor(0x89, 0xB4, 0xFA);
        let title_fg = crate::view::RGBColor(0xA6, 0xE3, 0xA1);

        let border_colors = Colors::Custom(border_fg, dark_bg);
        let text_colors = Colors::Custom(light_fg, dark_bg);
        let key_colors = Colors::Custom(key_fg, dark_bg);
        let title_colors = Colors::Custom(title_fg, dark_bg);

        // Top border with title
        self.print(
            &Position {
                line: row0,
                offset: col0,
            },
            Style::Default,
            border_colors,
            "╭",
        );
        if title.is_empty() {
            for c in 1..box_w.saturating_sub(1) {
                self.print(
                    &Position {
                        line: row0,
                        offset: col0 + c,
                    },
                    Style::Default,
                    border_colors,
                    "─",
                );
            }
        } else {
            let t_str = format!(" {} ", title);
            let t_len = t_str.graphemes(true).count();
            let max_t_len = box_w.saturating_sub(2);
            let display_title: String = if t_len > max_t_len {
                t_str.graphemes(true).take(max_t_len).collect()
            } else {
                t_str.clone()
            };
            for (gi, g) in display_title.graphemes(true).enumerate() {
                self.print(
                    &Position {
                        line: row0,
                        offset: col0 + 1 + gi,
                    },
                    Style::Bold,
                    title_colors,
                    g.to_string(),
                );
            }
            let dash_count = max_t_len.saturating_sub(display_title.graphemes(true).count());
            for c in 0..dash_count {
                self.print(
                    &Position {
                        line: row0,
                        offset: col0 + 1 + display_title.graphemes(true).count() + c,
                    },
                    Style::Default,
                    border_colors,
                    "─",
                );
            }
        }
        self.print(
            &Position {
                line: row0,
                offset: col0 + box_w - 1,
            },
            Style::Default,
            border_colors,
            "╮",
        );

        // Content rows
        let inner_w = box_w.saturating_sub(2 * BORDER_WIDTH);
        for (i, (key, desc)) in visible.iter().enumerate() {
            let row = row0 + 1 + i;

            // Left border
            self.print(
                &Position {
                    line: row,
                    offset: col0,
                },
                Style::Default,
                border_colors,
                "│",
            );

            // Clear content area
            for c in 0..inner_w {
                self.print(
                    &Position {
                        line: row,
                        offset: col0 + BORDER_WIDTH + c,
                    },
                    Style::Default,
                    text_colors,
                    " ",
                );
            }

            // Key (left-padded within key column)
            let key_start = col0 + BORDER_WIDTH + HORIZONTAL_PAD;
            let key_max = inner_w.saturating_sub(2 * HORIZONTAL_PAD);
            let key_display: String = key.graphemes(true).take(key_max).collect();
            // Right-pad the key to key_col_width
            let key_padded = format!("{:width$}", key_display, width = key_col_width);
            for (gi, grapheme) in key_padded.graphemes(true).enumerate() {
                if gi >= key_max {
                    break;
                }
                self.print(
                    &Position {
                        line: row,
                        offset: key_start + gi,
                    },
                    Style::Bold,
                    key_colors,
                    grapheme.to_string(),
                );
            }

            // Description
            let desc_start = key_start + key_col_width + GAP;
            let max_desc_g = inner_w.saturating_sub(2 * HORIZONTAL_PAD + key_col_width + GAP);
            let truncated_desc = truncate_popup_graphemes(desc, max_desc_g);
            for (gi, grapheme) in truncated_desc.graphemes(true).enumerate() {
                self.print(
                    &Position {
                        line: row,
                        offset: desc_start + gi,
                    },
                    Style::Default,
                    text_colors,
                    grapheme.to_string(),
                );
            }

            // Right border
            self.print(
                &Position {
                    line: row,
                    offset: col0 + box_w - 1,
                },
                Style::Default,
                border_colors,
                "│",
            );
        }

        // Bottom border
        let last_row = row0 + box_h - 1;
        self.print(
            &Position {
                line: last_row,
                offset: col0,
            },
            Style::Default,
            border_colors,
            "╰",
        );
        for c in 1..box_w.saturating_sub(1) {
            self.print(
                &Position {
                    line: last_row,
                    offset: col0 + c,
                },
                Style::Default,
                border_colors,
                "─",
            );
        }
        self.print(
            &Position {
                line: last_row,
                offset: col0 + box_w - 1,
            },
            Style::Default,
            border_colors,
            "╯",
        );
    }
}

fn truncate_popup_graphemes(s: &str, max: usize) -> String {
    let g: Vec<&str> = s.graphemes(true).collect();
    if g.len() <= max {
        s.to_string()
    } else if max == 0 {
        String::new()
    } else {
        let mut t: String = g[..max - 1].iter().copied().collect();
        t.push('~');
        t
    }
}

#[cfg(test)]
mod tests {
    use crate::models::application::Preferences;
    use crate::view::View;
    use scribe::{Buffer, Workspace};
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::rc::Rc;
    use std::sync::mpsc;

    #[test]
    fn print_buffer_initializes_renderer_with_cached_state() {
        let preferences = Rc::new(RefCell::new(Preferences::new(None)));
        let (tx, _) = mpsc::channel();
        let mut view = View::new(preferences, tx).unwrap();

        // Set up a Rust-categorized buffer.
        let mut workspace = Workspace::new(Path::new(".")).unwrap();
        let mut buffer = Buffer::new();
        buffer.id = Some(0);
        buffer.path = Some(PathBuf::from("rust.rs"));
        for _ in 0..200 {
            buffer.insert("line\n");
        }

        // Initialize the buffer's render cache, but get rid of the callback
        // so that we can test the cache without it being invalidated.
        view.initialize_buffer(&mut buffer).unwrap();
        // buffer.change_callback = None;
        workspace.add_buffer(buffer);

        // Scroll down enough to trigger caching during the render process.
        view.scroll_down(workspace.current_buffer.as_ref().unwrap(), 105)
            .unwrap();

        // Ensure there is nothing in the render cache for this buffer.
        let mut cache = view
            .get_render_cache(workspace.current_buffer.as_ref().unwrap())
            .unwrap();
        assert_eq!(cache.borrow().iter().count(), 0);

        // Draw the buffer.
        let mut presenter = view.build_presenter().unwrap();
        let data = workspace.current_buffer.as_ref().unwrap().data();
        presenter
            .print_buffer(
                workspace.current_buffer.as_ref().unwrap(),
                &data,
                &workspace.syntax_set,
                None,
                None,
            )
            .unwrap();

        // Ensure there is something in the render cache for this buffer.
        cache = view
            .get_render_cache(workspace.current_buffer.as_ref().unwrap())
            .unwrap();
        assert_ne!(cache.borrow().iter().count(), 0);
    }
}
