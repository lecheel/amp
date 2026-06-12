use crate::errors::*;
use crate::models::application::modes::ExMode;
use crate::view::RGBColor;
use crate::view::{Alignment, Colors, CursorType, StatusLineData, Style, View};
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;

const COMPLETION_COLUMNS: usize = 4;
const MODE_LABEL: &str = " CMD ";
const POPUP_BG: RGBColor = RGBColor(0x2C, 0x2C, 0x2C);
const POPUP_CMD_FG: RGBColor = RGBColor(0xA9, 0xDC, 0x76);
const POPUP_DIR_FG: RGBColor = RGBColor(0x61, 0xAF, 0xEF);
const POPUP_FILE_FG: RGBColor = RGBColor(0xAB, 0xB2, 0xBF);
const POPUP_SEL_FG: RGBColor = RGBColor(0xFF, 0xFF, 0xFF);
const POPUP_SEL_BG: RGBColor = RGBColor(0x4C, 0x78, 0xCC);

pub fn display(
    workspace: &mut Workspace,
    mode: &ExMode,
    view: &mut View,
    error: &Option<Error>,
) -> Result<()> {
    let completion = view.completion.clone();
    let data = workspace
        .current_buffer
        .as_ref()
        .map(|buf| buf.data())
        .unwrap_or_default();
    let mut presenter = view.build_presenter()?;
    if let Some(buf) = workspace.current_buffer.as_ref() {
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    }
    let terminal_height = presenter.height();
    let terminal_width = presenter.width();
    let status_line_y = terminal_height - 1;
    let completions = &mode.completions;
    let selection = mode.completion_selection;
    if !completions.is_empty() {
        let col_width = terminal_width / COMPLETION_COLUMNS;
        let row_count = (completions.len() + COMPLETION_COLUMNS - 1) / COMPLETION_COLUMNS;
        let max_rows = status_line_y.saturating_sub(1);
        let visible_rows = row_count.min(max_rows);
        let popup_start_y = status_line_y.saturating_sub(visible_rows);
        for row in 0..visible_rows {
            presenter.print(
                &Position {
                    line: popup_start_y + row,
                    offset: 0,
                },
                Style::Default,
                Colors::Custom(POPUP_BG, POPUP_BG),
                " ".repeat(terminal_width),
            );
            for col in 0..COMPLETION_COLUMNS {
                let idx = row * COMPLETION_COLUMNS + col;
                if idx >= completions.len() {
                    break;
                }
                let is_selected = selection == Some(idx);
                let x = col * col_width;
                let y = popup_start_y + row;
                let (fg, bg) = if is_selected {
                    (POPUP_SEL_FG, POPUP_SEL_BG)
                } else {
                    let fg = if completions[idx].display.ends_with('/') {
                        POPUP_DIR_FG
                    } else if completions[idx].display.starts_with(':') {
                        POPUP_CMD_FG
                    } else {
                        POPUP_FILE_FG
                    };
                    (fg, POPUP_BG)
                };
                let inner_width = col_width.saturating_sub(1);
                let label =
                    truncate_graphemes(&completions[idx].display, inner_width.saturating_sub(1));
                let label_len = label.graphemes(true).count();
                let pad = inner_width.saturating_sub(label_len);
                let cell = format!(" {}{}", label, " ".repeat(pad));
                presenter.print(
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
    if let Some(ref completion) = completion {
        let cursor_offset =
            MODE_LABEL.graphemes(true).count() + format!(" {}", mode.input).graphemes(true).count();
        let anchor = Position {
            line: status_line_y,
            offset: cursor_offset,
        };
        presenter.print_completion_popup(completion, anchor);
    }
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
                alignment: Alignment::Left,
            },
            StatusLineData {
                content: input_display,
                style: Style::Default,
                colors: Colors::Focused,
                alignment: Alignment::Expand,
            },
        ]);
    }
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
