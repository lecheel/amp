use crate::commands::Result;
use crate::errors::*;
use crate::models::application::Application;
use scribe::buffer::Position;

pub fn page_down(app: &mut Application) -> Result {
    let page_size = app.view.height().saturating_sub(2);
    {
        let buffer = app
            .workspace
            .current_buffer
            .as_mut()
            .context("No active buffer")?;
        let max_line = buffer.line_count().saturating_sub(1);
        let target_line = buffer.cursor.line.saturating_add(page_size).min(max_line);
        buffer.cursor.move_to(Position {
            line: target_line,
            offset: buffer.cursor.offset,
        });
    }
    {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .context("No active buffer")?;
        // Explicitly scroll the view down by a full page
        app.view.scroll_down(buffer, page_size)?;
        app.view.scroll_to_cursor(buffer)?;
    }
    Ok(())
}

pub fn page_up(app: &mut Application) -> Result {
    let page_size = app.view.height().saturating_sub(2);
    {
        let buffer = app
            .workspace
            .current_buffer
            .as_mut()
            .context("No active buffer")?;
        let target_line = buffer.cursor.line.saturating_sub(page_size);
        buffer.cursor.move_to(Position {
            line: target_line,
            offset: buffer.cursor.offset,
        });
    }
    {
        let buffer = app
            .workspace
            .current_buffer
            .as_ref()
            .context("No active buffer")?;
        // Explicitly scroll the view up by a full page
        app.view.scroll_up(buffer, page_size)?;
        app.view.scroll_to_cursor(buffer)?;
    }
    Ok(())
}

pub fn scroll_up(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    app.view.scroll_up(buffer, 10)?;
    Ok(())
}

pub fn scroll_down(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    app.view.scroll_down(buffer, 10)?;
    Ok(())
}

pub fn scroll_to_cursor(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    app.view.scroll_to_cursor(buffer)?;
    Ok(())
}

pub fn scroll_cursor_to_center(app: &mut Application) -> Result {
    let buffer = app
        .workspace
        .current_buffer
        .as_ref()
        .context(BUFFER_MISSING)?;
    app.view.scroll_to_center(buffer)?;
    Ok(())
}
