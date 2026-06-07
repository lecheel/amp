use crate::errors::*;
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
