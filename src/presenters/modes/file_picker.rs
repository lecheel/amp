use crate::errors::*;
use crate::models::application::modes::file_picker::FilePickerMode;
use crate::models::application::modes::SearchSelectMode;
use crate::presenters::current_buffer_status_line_data;
use crate::view::{Colors, CursorType, RGBColor, StatusLineData, Style, View};
use scribe::buffer::Position;
use scribe::Workspace;
use unicode_segmentation::UnicodeSegmentation;

pub fn display(
    workspace: &mut Workspace,
    mode: &mut FilePickerMode,
    view: &mut View,
    error: &Option<Error>,
) -> Result<()> {
    let data;
    let mut presenter = view.build_presenter()?;
    presenter.overlay = true; // Draw over the buffer instead of clearing

    // Calculate buffer status FIRST to avoid borrow checker issues
    let buffer_status = current_buffer_status_line_data(workspace);

    if let Some(buf) = workspace.current_buffer.as_ref() {
        data = buf.data();
        presenter.print_buffer(buf, &data, &workspace.syntax_set, None, None)?;
    }

    if let Some(e) = error {
        presenter.print_error(&e.to_string());
    } else {
        presenter.print_status_line(&[
            StatusLineData {
                content: String::from(" FILE PICKER "),
                style: Style::Default,
                colors: Colors::Inverted,
            },
            buffer_status,
        ]);
    }

    // Popup Sizing & Centering
    let box_w = std::cmp::max((presenter.width() as f32 * 0.6).ceil() as usize, 40);
    const FIXED_BODY_LINES: usize = 9;

    // Layout: Top border(1) + Title(1) + Sep(1) + Input(1) + Sep(1) + Items(9) + Bottom border(1)
    let box_h = FIXED_BODY_LINES + 6;

    // Center vertically and horizontally
    let row0 = (presenter.height().saturating_sub(1).saturating_sub(box_h)) / 2;
    let col0 = (presenter.width().saturating_sub(box_w)) / 2;

    // inner_w is the area inside the borders, minus 2 spaces for left/right padding
    let inner_w = box_w.saturating_sub(4);

    // Colors
    let border_fg = RGBColor(0x58, 0x5C, 0x6E);
    let dark_bg = RGBColor(0x1E, 0x1E, 0x2E);
    let light_fg = RGBColor(0xC0, 0xCA, 0xF5);
    let dir_fg = RGBColor(0x89, 0xB4, 0xFA);
    let sel_fg = RGBColor(0xFF, 0xFF, 0xFF);
    let sel_bg = RGBColor(0x4C, 0x78, 0xCC);

    let border_colors = Colors::Custom(border_fg, dark_bg);
    let text_colors = Colors::Custom(light_fg, dark_bg);
    let title_colors = Colors::Custom(dir_fg, dark_bg);

    // Helper to draw horizontal line
    let draw_hline = |presenter: &mut crate::view::Presenter,
                      row: usize,
                      col0: usize,
                      box_w: usize,
                      border_colors: Colors| {
        presenter.print(
            &Position {
                line: row,
                offset: col0,
            },
            Style::Default,
            border_colors,
            "+",
        );
        for c in 1..box_w.saturating_sub(1) {
            presenter.print(
                &Position {
                    line: row,
                    offset: col0 + c,
                },
                Style::Default,
                border_colors,
                "-",
            );
        }
        presenter.print(
            &Position {
                line: row,
                offset: col0 + box_w - 1,
            },
            Style::Default,
            border_colors,
            "+",
        );
    };

    // Helper to draw a padded line inside the box with specific background colors
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
            "|",
        );

        // Format with 1 space padding on each side
        let padded = format!(" {:width$} ", text, width = inner_w);

        // Print the padded string, applying the background color to the entire line
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
            "|",
        );
    };

    // 1. Top border
    draw_hline(&mut presenter, row0, col0, box_w, border_colors);

    // 2. Title (Current Directory)
    let title_row = row0 + 1;
    let title_str = mode.current_dir.to_string_lossy().to_string();
    print_line(
        &mut presenter,
        title_row,
        &title_str,
        Style::Bold,
        title_colors,
    );

    // 3. Separator
    draw_hline(&mut presenter, row0 + 2, col0, box_w, border_colors);

    // 4. Input Row (on top!)
    let input_row = row0 + 3;
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

    // 5. Separator
    draw_hline(&mut presenter, row0 + 4, col0, box_w, border_colors);

    // 6. Items (Fixed height of 9 lines, padded if empty)
    let results = mode.results().collect::<Vec<_>>();
    for i in 0..FIXED_BODY_LINES {
        let row = row0 + 5 + i;

        if i < results.len() {
            let entry = results[i];
            let is_dir = entry.0.is_dir();
            let name = entry
                .0
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let display_str = if is_dir { format!("{}/", name) } else { name };

            let (colors, style) = if i == mode.selected_index() {
                (Colors::Custom(sel_fg, sel_bg), Style::Bold)
            } else if is_dir {
                (Colors::Custom(dir_fg, dark_bg), Style::Default)
            } else {
                (text_colors, Style::Default)
            };

            print_line(&mut presenter, row, &display_str, style, colors);
        } else {
            // Draw empty padded line to maintain box size
            print_line(&mut presenter, row, "", Style::Default, text_colors);
        }
    }

    // 7. Bottom border
    let bottom_row = row0 + 5 + FIXED_BODY_LINES;
    draw_hline(&mut presenter, bottom_row, col0, box_w, border_colors);

    // 8. Cursor positioning
    if mode.insert_mode() {
        // The string is "> query", which inside the box is " > query ".
        // col0: '|'
        // col0 + 1: ' ' (padding)
        // col0 + 2: '>'
        // col0 + 3: ' ' (space after >)
        // col0 + 4: first char of query
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
