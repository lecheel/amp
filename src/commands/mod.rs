use crate::models::application::Application;
use std::collections::HashMap;

pub mod alias;
pub mod application;
pub mod buffer;
pub mod buffer_list;
pub mod confirm;
pub mod cursor;
pub mod ex;
pub mod git;
pub mod jump;
pub mod line_jump;
pub mod open;
pub mod path;
pub mod preferences;
pub mod search;
pub mod search_select;
pub mod selection;
pub mod view;
pub mod workspace;

pub type Command = fn(&mut Application) -> Result;
pub type Result = anyhow::Result<()>;

pub fn hash_map() -> HashMap<&'static str, Command> {
    let mut map: HashMap<&'static str, Command> = include!(concat!(env!("OUT_DIR"), "/hash_map"));

    map.insert("application::nop", application::nop);
    map.insert(
        "buffer_list::open_under_cursor",
        buffer_list::open_under_cursor,
    );

    // Ex mode commands
    map.insert("ex::push_char", ex::push_char);
    map.insert("ex::pop_char", ex::pop_char);
    map.insert("ex::accept_input", ex::accept_input);
    map.insert("ex::previous_history", ex::previous_history);
    map.insert("ex::next_history", ex::next_history);
    map.insert("ex::navigate_up", ex::navigate_up);
    map.insert("ex::navigate_down", ex::navigate_down);
    map.insert("ex::navigate_left", ex::navigate_left);
    map.insert("ex::navigate_right", ex::navigate_right);
    map.insert("ex::select_next_completion", ex::select_next_completion);
    map.insert(
        "ex::select_previous_completion",
        ex::select_previous_completion,
    );
    map.insert("ex::apply_completion", ex::apply_completion);
    map.insert("ex::complete", ex::complete);
    map.insert("ex::apply_completion", ex::apply_completion);
    map.insert("ex::complete", ex::complete);

    map
}
