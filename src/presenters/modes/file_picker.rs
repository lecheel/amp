use crate::errors::*;
use crate::models::application::modes::file_picker::FilePickerMode;
use crate::models::application::modes::SearchSelectMode;
use crate::presenters::standard_status_line;
use crate::view::{Colors, CursorType, RGBColor, Style, View};
use git2::Repository;
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;

pub fn display(
    workspace: &mut Workspace,
    mode: &mut FilePickerMode,
    view: &mut View,
    repo: &Option<Repository>,
    error: &Option<Error>,
) -> Result<()> {
    let data;
    let status_entries =
        standard_status_line("FILE PICKER", Colors::Inverted, workspace, view, repo);
    let mut presenter = view.build_presenter()?;
    presenter.overlay = true;

    if let Some(buf) = workspace.current_buffer.as_ref() {
        data = buf.data();
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    }

    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&status_entries);
    }

    let box_w = std::cmp::max((presenter.width() as f32 * 0.6).ceil() as usize, 40);
    const FIXED_BODY_LINES: usize = 12;
    let box_h = FIXED_BODY_LINES + 3;
    let row0 = (presenter.height().saturating_sub(1).saturating_sub(box_h)) / 2;
    let col0 = (presenter.width().saturating_sub(box_w)) / 2;
    let inner_w = box_w.saturating_sub(4);

    let border_fg = RGBColor(0x58, 0x5C, 0x6E);
    let dark_bg = RGBColor(0x1E, 0x1E, 0x2E);
    let light_fg = RGBColor(0xC0, 0xCA, 0xF5);
    let dir_fg = RGBColor(0x89, 0xB4, 0xFA);
    let sel_fg = RGBColor(0xFF, 0xFF, 0xFF);
    let sel_bg = RGBColor(0x4C, 0x78, 0xCC);

    let border_colors = Colors::Custom(border_fg, dark_bg);
    let text_colors = Colors::Custom(light_fg, dark_bg);
    let title_colors = Colors::Custom(dir_fg, dark_bg);

    let print_line = |presenter: &mut crate::view::Presenter,
                      row: usize,
                      text: &str,
                      text_style: Style,
                      colors: Colors| {
        presenter.print(
            &Position {
                line: row,
                offset: col0,
            },
            Style::Default,
            border_colors,
            "│",
        );
        let padded = format!(" {:width$} ", text, width = inner_w);
        for (gi, grapheme) in padded.graphemes(true).enumerate().take(inner_w + 2) {
            presenter.print(
                &Position {
                    line: row,
                    offset: col0 + 1 + gi,
                },
                text_style,
                colors,
                grapheme.to_string(),
            );
        }
        presenter.print(
            &Position {
                line: row,
                offset: col0 + box_w - 1,
            },
            Style::Default,
            border_colors,
            "│",
        );
    };

    presenter.print(
        &Position {
            line: row0,
            offset: col0,
        },
        Style::Default,
        border_colors,
        "╭",
    );
    let title_str = format!(" {} ", mode.current_dir.to_string_lossy());
    let title_glen = title_str.graphemes(true).count();
    let max_title_len = box_w.saturating_sub(2);
    if title_glen >= max_title_len {
        let truncated: String = title_str.graphemes(true).take(max_title_len).collect();
        for (gi, g) in truncated.graphemes(true).enumerate() {
            presenter.print(
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
        for (gi, g) in title_str.graphemes(true).enumerate() {
            presenter.print(
                &Position {
                    line: row0,
                    offset: col0 + 1 + gi,
                },
                Style::Bold,
                title_colors,
                g.to_string(),
            );
        }
        let dash_count = max_title_len.saturating_sub(title_glen);
        for c in 0..dash_count {
            presenter.print(
                &Position {
                    line: row0,
                    offset: col0 + 1 + title_glen + c,
                },
                Style::Default,
                border_colors,
                "─",
            );
        }
    }
    presenter.print(
        &Position {
            line: row0,
            offset: col0 + box_w - 1,
        },
        Style::Default,
        border_colors,
        "╮",
    );

    let input_row = row0 + 1;
    let input_str = format!("> {}", mode.query());
    let input_colors = if mode.insert_mode() {
        Colors::Insert
    } else {
        Colors::Inverted
    };
    print_line(
        &mut presenter,
        input_row,
        &input_str,
        Style::Bold,
        input_colors,
    );

    let selected = mode.selected_index();
    let _result_count = mode.results().count();
    let new_scroll_offset = if selected < mode.scroll_offset {
        selected
    } else if selected >= mode.scroll_offset + FIXED_BODY_LINES {
        selected + 1 - FIXED_BODY_LINES
    } else {
        mode.scroll_offset
    };
    mode.scroll_offset = new_scroll_offset;
    let scroll_offset = new_scroll_offset;
    let results = mode.results().collect::<Vec<_>>();
    for i in 0..FIXED_BODY_LINES {
        let row = row0 + 2 + i;
        let result_index = scroll_offset + i;
        if result_index < results.len() {
            let entry = results[result_index];
            let is_dir = entry.0.is_dir();
            let name = entry
                .0
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let display_str = if is_dir { format!("{}/", name) } else { name };
            let (colors, style) = if result_index == selected {
                (Colors::Custom(sel_fg, sel_bg), Style::Bold)
            } else if is_dir {
                (Colors::Custom(dir_fg, dark_bg), Style::Default)
            } else {
                (text_colors, Style::Default)
            };
            print_line(&mut presenter, row, &display_str, style, colors);
        } else {
            print_line(&mut presenter, row, "", Style::Default, text_colors);
        }
    }
    let bottom_row = row0 + 2 + FIXED_BODY_LINES;
    presenter.print(
        &Position {
            line: bottom_row,
            offset: col0,
        },
        Style::Default,
        border_colors,
        "╰",
    );
    for c in 1..box_w.saturating_sub(1) {
        presenter.print(
            &Position {
                line: bottom_row,
                offset: col0 + c,
            },
            Style::Default,
            border_colors,
            "─",
        );
    }
    presenter.print(
        &Position {
            line: bottom_row,
            offset: col0 + box_w - 1,
        },
        Style::Default,
        border_colors,
        "╯",
    );

    if mode.insert_mode() {
        let cursor_offset = col0 + 4 + mode.query().graphemes(true).count();
        presenter.set_cursor(Some(Position {
            line: input_row,
            offset: cursor_offset,
        }));
        presenter.set_cursor_type(CursorType::BlinkingBar);
    } else {
        presenter.set_cursor(None);
    }
    presenter.present()?;
    Ok(())
}
