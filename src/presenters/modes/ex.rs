use crate::errors::*;
use crate::models::application::modes::ExMode;
use crate::view::{Colors, CursorType, StatusLineData, Style, View};
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;

const COMPLETION_COLUMNS: usize = 4;
const MODE_LABEL: &str = " CMD ";

pub fn display(
    workspace: &mut Workspace,
    mode: &ExMode,
    view: &mut View,
    error: &Option<Error>,
) -> Result<()> {
    let data = workspace
        .current_buffer
        .as_ref()
        .map(|buf| buf.data())
        .unwrap_or_default();

    let mut presenter = view.build_presenter()?;

    if let Some(buf) = workspace.current_buffer.as_ref() {
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    }

    // Calculate popup dimensions
    let terminal_height = presenter.height();
    let terminal_width = presenter.width();
    let status_line_y = terminal_height - 1;
    let completions = &mode.completions;
    let selection = mode.completion_selection;

    // Render completion popup above status line if there are candidates
    if !completions.is_empty() {
        let col_width = terminal_width / COMPLETION_COLUMNS;
        let row_count = (completions.len() + COMPLETION_COLUMNS - 1) / COMPLETION_COLUMNS;

        let max_rows = status_line_y.saturating_sub(1);
        let visible_rows = row_count.min(max_rows);
        let popup_start_y = status_line_y.saturating_sub(visible_rows);

        for row in 0..visible_rows {
            for col in 0..COMPLETION_COLUMNS {
                let idx = row * COMPLETION_COLUMNS + col;
                if idx >= completions.len() {
                    break;
                }

                let is_selected = selection == Some(idx);
                let x = col * col_width;
                let y = popup_start_y + row;

                // Use the display field for rendering
                let text =
                    truncate_graphemes(&completions[idx].display, col_width.saturating_sub(1));

                let colors = if is_selected {
                    Colors::Inverted
                } else if completions[idx].display.ends_with('/') {
                    // Directories get a different color
                    Colors::CustomForeground(crate::view::RGBColor(0x61, 0xAF, 0xEF))
                // blue
                } else if completions[idx].display.starts_with(':') {
                    // Commands get green
                    Colors::CustomForeground(crate::view::RGBColor(0xA9, 0xDC, 0x76))
                // green
                } else {
                    // Files get white/default
                    Colors::CustomForeground(crate::view::RGBColor(0xAB, 0xB2, 0xBF))
                    // silver
                };

                // Clear the cell area first
                presenter.print(
                    &Position { line: y, offset: x },
                    Style::Default,
                    Colors::Default,
                    " ".repeat(col_width),
                );

                // Print the completion text
                presenter.print(
                    &Position { line: y, offset: x },
                    if is_selected {
                        Style::Bold
                    } else {
                        Style::Default
                    },
                    colors,
                    text,
                );
            }
        }
    }

    // Render status line
    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        let mode_display = MODE_LABEL.to_string();
        let input_display = format!(" {}", mode.input);

        presenter.print_status_line(&[
            StatusLineData {
                content: mode_display,
                style: Style::Default,
                colors: Colors::Inverted,
            },
            StatusLineData {
                content: input_display,
                style: Style::Default,
                colors: Colors::Focused,
            },
        ]);
    }

    // Position cursor at the end of the input prompt
    let cursor_offset =
        MODE_LABEL.graphemes(true).count() + format!(" {}", mode.input).graphemes(true).count();
    let cursor_line = terminal_height - 1;
    presenter.set_cursor(Some(Position {
        line: cursor_line,
        offset: cursor_offset,
    }));

    presenter.set_cursor_type(CursorType::BlinkingBar);
    presenter.present()?;

    Ok(())
}

/// Truncates a string to at most `max_graphemes` grapheme clusters,
/// appending "~" if truncated.
fn truncate_graphemes(s: &str, max_graphemes: usize) -> String {
    let graphemes: Vec<&str> = s.graphemes(true).collect();
    if graphemes.len() <= max_graphemes {
        s.to_string()
    } else if max_graphemes == 0 {
        String::new()
    } else {
        let mut truncated: String = graphemes[..max_graphemes - 1].iter().copied().collect();
        truncated.push('~');
        truncated
    }
}
