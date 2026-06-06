use crate::models::application::Application;
use std::collections::HashMap;

pub mod alias;
pub mod application;
pub mod buffer;
pub mod confirm;
pub mod cursor;
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

    // Vim-style command mode aliases
    map.insert("bn", workspace::next_buffer);
    map.insert("bp", workspace::prev_buffer);
    map.insert("bd", buffer::close);
    map.insert("w", buffer::save);
    map.insert("q", application::exit);
    map.insert("wq", alias::save_and_exit);
    map.insert("q!", alias::force_exit);
    map
}
