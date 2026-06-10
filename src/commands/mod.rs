use crate::models::application::Application;
use std::collections::HashMap;

pub mod alias;
pub mod application;
pub mod buffer;
pub mod buffer_list;
pub mod completion;
pub mod confirm;
pub mod cursor;
pub mod ex;
pub mod fd;
pub mod file_picker;
pub mod git;
pub mod jump;
pub mod line_jump;
pub mod mru;
pub mod open;
pub mod path;
pub mod preferences;
pub mod repeat;
pub mod rg;
pub mod search;
pub mod search_select;
pub mod select_block;
pub mod selection;
pub mod tag;
pub mod view;
pub mod workspace;

pub type Command = fn(&mut Application) -> Result;
pub type Result = anyhow::Result<()>;

pub fn hash_map() -> HashMap<&'static str, Command> {
    let mut map: HashMap<&'static str, Command> = include!(concat!(env!("OUT_DIR"), "/hash_map"));

    map.insert("application::nop", application::nop);
    map.insert("tag::tag_under_cursor", tag::tag_under_cursor);
    map.insert("tag::tag_back", tag::tag_back);
    map.insert("tag::gentags", tag::gentags);
    map.insert("buffer_list::accept", buffer_list::accept);
    map.insert(
        "application::switch_to_buffer_list_mode",
        buffer_list::switch_to_buffer_list_mode,
    );

    // Ex mode commands
    map.insert("buffer::fmt_save", buffer::fmt_save);
    map.insert("view::page_down", view::page_down);
    map.insert("view::page_up", view::page_up);
    map.insert("ex::push_char", ex::push_char);
    map.insert("ex::pop_char", ex::pop_char);
    map.insert("ex::accept_input", ex::accept_input);
    map.insert("ex::previous_history", ex::previous_history);
    map.insert("ex::next_history", ex::next_history);
    map.insert("ex::navigate_up", ex::navigate_up);
    map.insert("ex::navigate_down", ex::navigate_down);
    map.insert("ex::navigate_left", ex::navigate_left);
    map.insert("ex::navigate_right", ex::navigate_right);
    map.insert("git::revert_hunk", git::revert_hunk);
    map.insert("git::show_hunk_diff", git::show_hunk_diff);
    map.insert("cursor::match_bracket", cursor::match_bracket);
    map.insert(
        "file_picker::switch_to_file_picker_mode",
        file_picker::switch_to_file_picker_mode,
    );
    map.insert("file_picker::accept", file_picker::accept);
    map.insert("file_picker::navigate_up", file_picker::navigate_up);
    map.insert("fd::switch_to_fd_mode", fd::switch_to_fd_mode_no_filter);
    map.insert("fd::accept", fd::accept);
    map.insert(
        "completion::complete_from_buffer",
        completion::complete_from_buffer,
    );
    map.insert("completion::select_next", completion::select_next);
    map.insert("completion::select_previous", completion::select_previous);
    map.insert("completion::accept", completion::accept);
    map.insert("completion::cancel", completion::cancel);
    map.insert("mru::accept", mru::accept);
    map.insert("ex::select_next_completion", ex::select_next_completion);
    map.insert(
        "ex::select_previous_completion",
        ex::select_previous_completion,
    );
    map.insert("ex::apply_completion", ex::apply_completion);
    map.insert("ex::complete", ex::complete);
    map.insert("ex::apply_completion", ex::apply_completion);
    map.insert("ex::complete", ex::complete);
    map.insert("rg::switch_to_last_rg", rg::switch_to_last_rg);
    map.insert("rg::next_result", rg::next_result);
    map.insert("rg::prev_result", rg::prev_result);
    map.insert(
        "application::open_under_cursor",
        application::open_under_cursor,
    );

    // Block selection commands (ensure registration even if build script misses them)
    map.insert(
        "select_block::switch_to_select_block_mode",
        select_block::switch_to_select_block_mode,
    );
    map.insert("select_block::block_insert", select_block::block_insert);
    map.insert("select_block::block_append", select_block::block_append);
    map.insert("select_block::insert_char", select_block::insert_char);
    map.insert("select_block::backspace", select_block::backspace);
    map.insert("select_block::insert_newline", select_block::insert_newline);
    map.insert("select_block::insert_tab", select_block::insert_tab);
    map.insert("select_block::apply_and_exit", select_block::apply_and_exit);
    map.insert("select_block::delete", select_block::delete);
    map.insert("select_block::copy", select_block::copy);
    map.insert(
        "select_block::copy_and_delete",
        select_block::copy_and_delete,
    );
    map.insert("select_block::change", select_block::change);

    map
}
