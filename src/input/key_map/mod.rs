use crate::commands::{self, Command};
use crate::errors::*;
use crate::input::Key;
use smallvec::SmallVec;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use yaml_rust::yaml::{Hash, Yaml, YamlLoader};

pub enum LeaderNode {
    Command(SmallVec<[Command; 4]>),
    Subtree(HashMap<Key, LeaderNode>),
}

pub struct KeyMap(
    HashMap<String, HashMap<Key, SmallVec<[Command; 4]>>>,
    HashMap<Key, LeaderNode>, // leader tree, field .1
);

impl KeyMap {
    /// Walk the leader tree with the given key sequence.
    /// Returns None if no match, Some(Command) if terminal,
    /// and the bool indicates whether the prefix is still viable.
    pub fn leader_lookup(&self, keys: &[Key]) -> LeaderLookup {
        let mut current = &self.1;
        for (i, key) in keys.iter().enumerate() {
            match current.get(key) {
                None => return LeaderLookup::NoMatch,
                Some(LeaderNode::Command(cmds)) => {
                    if i == keys.len() - 1 {
                        return LeaderLookup::Found(cmds.clone());
                    } else {
                        return LeaderLookup::NoMatch; // tried to go deeper into a leaf
                    }
                }
                Some(LeaderNode::Subtree(sub)) => {
                    if i == keys.len() - 1 {
                        return LeaderLookup::Prefix; // valid prefix, keep waiting
                    }
                    current = sub;
                }
            }
        }
        LeaderLookup::Prefix
    }

    /// Parses a Yaml tree of modes and their keybindings into a complete keymap.
    ///
    /// e.g.
    ///
    ///  normal:
    ///     k: "cursor::move_up"
    ///
    /// becomes this HashMap entry:
    ///
    ///   "normal" => { Key::Char('k') => commands::cursor::move_up }
    ///
    pub fn from(keymap_data: &Hash) -> Result<KeyMap> {
        let mut keymap = HashMap::new();
        let commands = commands::hash_map();
        let mut leader_tree = HashMap::new();

        for (yaml_mode, yaml_key_bindings) in keymap_data {
            let mode = yaml_mode.as_str().context("Mode key must be a string")?;

            if mode == "leader" {
                leader_tree = parse_leader_tree(yaml_key_bindings, &commands)?;
                continue;
            }

            let key_bindings = parse_mode_key_bindings(yaml_key_bindings, &commands)
                .with_context(|| format!("Failed to parse keymaps for \"{mode}\" mode"))?;
            keymap.insert(mode.to_string(), key_bindings);
        }

        Ok(KeyMap(keymap, leader_tree))
    }

    /// Searches the keymap for the specified key.
    /// Character keys will fall back to wildcard character bindings
    /// if the specific character binding cannot be found.
    ///
    pub fn commands_for(&self, mode: &str, key: &Key) -> Option<SmallVec<[Command; 4]>> {
        self.0
            .get(mode)
            .and_then(|mode_keymap| {
                if let Key::Char(_) = *key {
                    // Look for a command for this specific character, falling
                    // back to another search for a wildcard character binding.
                    mode_keymap
                        .get(key)
                        .or_else(|| mode_keymap.get(&Key::AnyChar))
                } else {
                    mode_keymap.get(key)
                }
            })
            .map(|commands| (*commands).clone())
    }

    /// Loads the default keymap from a static
    /// YAML document injected during the build.
    pub fn default() -> Result<KeyMap> {
        let default_keymap_data = YamlLoader::load_from_str(KeyMap::default_data())
            .context("Couldn't parse default keymap")?
            .into_iter()
            .next()
            .context("Couldn't locate a document in the default keymap")?;

        KeyMap::from(default_keymap_data.as_hash().unwrap())
    }

    /// Returns the default YAML keymap data as a string.
    pub fn default_data() -> &'static str {
        include_str!("default.yml")
    }

    /// Merges each of the passed key map's modes, consuming them in the process.
    /// Note: the mode must exist to be merged; unmatched modes are discarded.
    ///
    /// e.g.
    ///
    /// normal:
    ///     k: "cursor::move_up"
    ///
    /// merged with:
    ///
    /// normal:
    ///     j: "cursor::move_down"
    /// unknown:
    ///     l: "cursor::move_right"
    ///
    /// becomes this:
    ///
    ///   "normal" => {
    ///       Key::Char('k') => commands::cursor::move_up
    ///       Key::Char('j') => commands::cursor::move_down
    ///   }
    ///
    pub fn merge(&mut self, mut key_map: KeyMap) {
        // Merge mode keybindings
        // Use the entry API to ensure the mode HashMap exists before inserting,
        // allowing user configs to define entirely new modes.
        for (mode, other_key_bindings) in key_map.0.iter_mut() {
            let key_bindings = self.0.entry(mode.to_string()).or_insert_with(HashMap::new);
            for (key, command) in other_key_bindings.drain() {
                key_bindings.insert(key, command);
            }
        }

        // Merge leader tree
        // Insert/replace top-level leader keys from the user config.
        for (key, node) in key_map.1.drain() {
            self.1.insert(key, node);
        }
    }
}

pub enum LeaderLookup {
    Found(SmallVec<[Command; 4]>),
    Prefix,  // valid so far, wait for more keys
    NoMatch, // cancel
}

fn parse_leader_tree(
    yaml: &Yaml,
    commands: &HashMap<&str, Command>,
) -> Result<HashMap<Key, LeaderNode>> {
    let hash = yaml.as_hash().context("Leader tree node must be a hash")?;

    let mut map = HashMap::new();
    for (k, v) in hash {
        let key = parse_key(k.as_str().context("Leader key must be a string")?)?;
        let node = match v {
            // leaf: "gr: git::copy_remote_url"
            Yaml::String(cmd) => {
                let f = commands
                    .get(cmd.as_str())
                    .with_context(|| format!("Unknown command: {cmd}"))?;
                let mut sv = SmallVec::new();
                sv.push(*f);
                LeaderNode::Command(sv)
            }
            // leaf: list of commands
            Yaml::Array(arr) => {
                let mut sv = SmallVec::new();
                for item in arr {
                    let s = item.as_str().context("Command must be string")?;
                    sv.push(
                        *commands
                            .get(s)
                            .with_context(|| format!("Unknown command: {s}"))?,
                    );
                }
                LeaderNode::Command(sv)
            }
            // subtree: nested hash
            Yaml::Hash(_) => LeaderNode::Subtree(parse_leader_tree(v, commands)?),
            other => bail!("Unexpected leader node: {other:?}"),
        };
        map.insert(key, node);
    }
    Ok(map)
}

/// Parses the key bindings for a particular mode.
///
/// e.g.
///
///   k: "cursor::move_up"
///
/// becomes this HashMap entry:
///
///   Key::Char('k') => [commands::cursor::move_up]
///
fn parse_mode_key_bindings(
    mode: &Yaml,
    commands: &HashMap<&str, Command>,
) -> Result<HashMap<Key, SmallVec<[Command; 4]>>> {
    let mode_key_bindings = mode
        .as_hash()
        .context("Keymap mode config didn't return a hash of key bindings")?;

    let mut key_bindings = HashMap::new();
    for (yaml_key, yaml_command) in mode_key_bindings {
        // Parse modifier/character from key component.
        let key = parse_key(
            yaml_key
                .as_str()
                .with_context(|| "A keymap key couldn't be parsed as a string".to_string())?,
        )?;

        let mut key_commands = SmallVec::new();

        // Parse and find command reference from command component.
        match *yaml_command {
            Yaml::String(ref command) => {
                let command_string = command.as_str();

                key_commands.push(*commands.get(&command_string).with_context(|| {
                    format!("Keymap command \"{command_string}\" doesn't exist")
                })?);
            }
            Yaml::Array(ref command_array) => {
                for command in command_array {
                    let command_string = command.as_str().with_context(|| {
                        format!("Keymap command \"{command:?}\" couldn't be parsed as a string")
                    })?;

                    key_commands.push(*commands.get(command_string).with_context(|| {
                        format!("Keymap command \"{command_string}\" doesn't exist")
                    })?);
                }
            }
            _ => bail!(format!(
                "Keymap command \"{:?}\" couldn't be parsed",
                yaml_command
            )),
        }

        // Add a key/command entry to the mapping.
        key_bindings.insert(key, key_commands);
    }

    Ok(key_bindings)
}

/// Parses a str-based key into its Key equivalent.
///
/// e.g.
///
///   ctrl-r becomes Key::Ctrl('r')
///
fn parse_key(data: &str) -> Result<Key> {
    let mut key_components = data.splitn(2, '-'); // <-- Change this line
    let component = key_components
        .next()
        .context("A keymap key is an empty string")?;

    if let Some(key) = key_components.next() {
        // We have a modifier-qualified key; get the key.
        let key_char = key
            .chars()
            .next()
            .with_context(|| format!("Keymap key \"{key}\" is invalid"))?;

        // Find the variant for the specified modifier.
        match component {
            "ctrl" => Ok(Key::Ctrl(key_char)),
            "alt" => Ok(Key::Alt(key_char)),
            _ => bail!(format!("Keymap modifier \"{}\" is invalid", component)),
        }
    } else {
        // No modifier; just get the key.
        Ok(match component {
            "space" => Key::Char(' '),
            "backspace" => Key::Backspace,
            "left" => Key::Left,
            "right" => Key::Right,
            "up" => Key::Up,
            "down" => Key::Down,
            "home" => Key::Home,
            "end" => Key::End,
            "page_up" => Key::PageUp,
            "page_down" => Key::PageDown,
            "delete" => Key::Delete,
            "insert" => Key::Insert,
            "escape" => Key::Esc,
            "tab" => Key::Tab,
            "enter" => Key::Enter,
            "f1" => Key::F1,
            "f2" => Key::F2,
            "f3" => Key::F3,
            "f4" => Key::F4,
            "f5" => Key::F5,
            "f6" => Key::F6,
            "f7" => Key::F7,
            "f8" => Key::F8,
            "f9" => Key::F9,
            "f10" => Key::F10,
            "f11" => Key::F11,
            "f12" => Key::F12,
            "_" => Key::AnyChar,
            _ => Key::Char(
                component
                    .chars()
                    .next()
                    .with_context(|| format!("Keymap key \"{component}\" is invalid"))?,
            ),
        })
    }
}

impl Deref for KeyMap {
    type Target = HashMap<String, HashMap<Key, SmallVec<[Command; 4]>>>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for KeyMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl From<KeyMap> for HashMap<String, HashMap<Key, SmallVec<[Command; 4]>>> {
    fn from(val: KeyMap) -> Self {
        val.0
    }
}

#[cfg(test)]
mod tests {
    use super::KeyMap;
    use crate::commands;
    use crate::input::Key;
    use yaml_rust::YamlLoader;

    #[test]
    fn keymap_correctly_parses_yaml_character_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  k: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0].as_hash().unwrap()).unwrap();

        let command = keymap
            .commands_for("normal", &Key::Char('k'))
            .expect("Keymap doesn't contain command");
        assert_eq!(
            (command[0] as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_parses_yaml_wildcard_character_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  _: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0].as_hash().unwrap()).unwrap();

        let characters = vec!['a', 'b', 'c'];
        for c in characters.into_iter() {
            let command = keymap
                .commands_for("normal", &Key::Char(c))
                .expect("Keymap doesn't contain command");
            assert_eq!(
                (command[0] as *const usize),
                (commands::cursor::move_up as *const usize)
            );
        }
    }

    #[test]
    fn keymap_correctly_prioritizes_character_over_wildcard_character_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  j: cursor::move_down\n  _: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0].as_hash().unwrap()).unwrap();

        let char_command = keymap
            .commands_for("normal", &Key::Char('j'))
            .expect("Keymap doesn't contain command");
        assert_eq!(
            (char_command[0] as *const usize),
            (commands::cursor::move_down as *const usize)
        );
        let wildcard_command = keymap
            .commands_for("normal", &Key::Char('a'))
            .expect("Keymap doesn't contain command");
        assert_eq!(
            (wildcard_command[0] as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_parses_yaml_control_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  ctrl-r: cursor::move_up";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0].as_hash().unwrap()).unwrap();

        let command = keymap
            .commands_for("normal", &Key::Ctrl('r'))
            .expect("Keymap doesn't contain command");
        assert_eq!(
            (command[0] as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_parses_yaml_keyword_keybindings() {
        let mappings = vec![
            (
                "normal:\n  space: cursor::move_up",
                Key::Char(' '),
                commands::cursor::move_up,
            ),
            (
                "normal:\n  backspace: cursor::move_up",
                Key::Backspace,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  left: cursor::move_up",
                Key::Left,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  right: cursor::move_up",
                Key::Right,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  up: cursor::move_up",
                Key::Up,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  down: cursor::move_up",
                Key::Down,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  home: cursor::move_up",
                Key::Home,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  end: cursor::move_up",
                Key::End,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  page_up: cursor::move_up",
                Key::PageUp,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  page_down: cursor::move_up",
                Key::PageDown,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  delete: cursor::move_up",
                Key::Delete,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  insert: cursor::move_up",
                Key::Insert,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  escape: cursor::move_up",
                Key::Esc,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  tab: cursor::move_up",
                Key::Tab,
                commands::cursor::move_up,
            ),
            (
                "normal:\n  enter: cursor::move_up",
                Key::Enter,
                commands::cursor::move_up,
            ),
        ];

        for (binding, key, command) in mappings {
            // Build the keymap
            let yaml = YamlLoader::load_from_str(binding).unwrap();
            let keymap = KeyMap::from(&yaml[0].as_hash().unwrap()).unwrap();

            let parsed_command = keymap
                .commands_for("normal", &key)
                .expect("Keymap doesn't contain command");
            assert_eq!(
                (parsed_command[0] as *const usize),
                (command as *const usize)
            );
        }
    }

    #[test]
    fn keymap_correctly_loads_default_keybindings() {
        // Build the keymap
        let keymap = KeyMap::default().unwrap();

        let command = keymap
            .commands_for("normal", &Key::Char('k'))
            .expect("Keymap doesn't contain command");
        assert_eq!(
            (command[0] as *const usize),
            (commands::cursor::move_up as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_merges_keybindings() {
        let yaml_data = "normal:\n  k: cursor::move_up\n  j: cursor::move_down";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let mut keymap = KeyMap::from(&yaml[0].as_hash().unwrap()).unwrap();

        let other_yaml_data = "normal:\n  k: cursor::move_left\n  l: cursor::move_right";
        let other_yaml = YamlLoader::load_from_str(other_yaml_data).unwrap();
        let other_keymap = KeyMap::from(&other_yaml[0].as_hash().unwrap()).unwrap();

        keymap.merge(other_keymap);

        let mut command = keymap
            .commands_for("normal", &Key::Char('j'))
            .expect("Keymap doesn't contain original command");
        assert_eq!(
            (command[0] as *const usize),
            (commands::cursor::move_down as *const usize)
        );

        command = keymap
            .commands_for("normal", &Key::Char('k'))
            .expect("Keymap doesn't contain overlapping command");
        assert_eq!(
            (command[0] as *const usize),
            (commands::cursor::move_left as *const usize)
        );

        command = keymap
            .commands_for("normal", &Key::Char('l'))
            .expect("Keymap doesn't contain other command");
        assert_eq!(
            (command[0] as *const usize),
            (commands::cursor::move_right as *const usize)
        );
    }

    #[test]
    fn keymap_correctly_parses_multiple_yaml_keybindings() {
        // Build the keymap
        let yaml_data = "normal:\n  ctrl-r:\n    - cursor::move_up\n    - cursor::move_down";
        let yaml = YamlLoader::load_from_str(yaml_data).unwrap();
        let keymap = KeyMap::from(&yaml[0].as_hash().unwrap()).unwrap();

        let command = keymap
            .commands_for("normal", &Key::Ctrl('r'))
            .expect("Keymap doesn't contain command");
        assert_eq!(
            (command[0] as *const usize),
            (commands::cursor::move_up as *const usize)
        );
        assert_eq!(
            (command[1] as *const usize),
            (commands::cursor::move_down as *const usize)
        );
    }
}
