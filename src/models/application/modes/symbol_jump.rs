use crate::errors::*;
use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
use crate::util::SelectableVec;
use fragment;
use fragment::matching::AsStr;
use scribe::buffer::{Position, Token, TokenSet};
use std::clone::Clone;
use std::fmt;
use std::iter::Iterator;
use std::slice::Iter;
use std::str::FromStr;
use syntect::highlighting::ScopeSelectors;

pub struct SymbolJumpMode {
    insert: bool,
    input: String,
    symbols: Vec<Symbol>,
    all_results: Vec<Symbol>, // ALL matches from search (not truncated)
    scroll_offset: usize,     // index into all_results of first visible item
    results: SelectableVec<Symbol>, // visible window only
    config: SearchSelectConfig,
}

#[derive(PartialEq, Debug)]
pub struct Symbol {
    pub token: String,
    pub position: Position,
}

impl fmt::Display for Symbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &self.token)
    }
}

impl Clone for Symbol {
    fn clone(&self) -> Symbol {
        Symbol {
            token: self.token.clone(),
            position: self.position,
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.token = source.token.clone();
        self.position = source.position;
    }
}

impl AsStr for Symbol {
    fn as_str(&self) -> &str {
        &self.token
    }
}

impl SymbolJumpMode {
    pub fn new(config: SearchSelectConfig) -> Result<SymbolJumpMode> {
        Ok(SymbolJumpMode {
            insert: true,
            input: String::new(),
            symbols: Vec::new(),
            all_results: Vec::new(),
            scroll_offset: 0,
            results: SelectableVec::new(Vec::new()),
            config,
        })
    }

    pub fn reset(&mut self, tokens: &TokenSet, config: SearchSelectConfig) -> Result<()> {
        self.insert = true;
        self.input.clear();
        self.symbols = symbols(tokens.iter().context(BUFFER_PARSE_FAILED)?);
        self.all_results = Vec::new();
        self.scroll_offset = 0;
        self.results = SelectableVec::new(Vec::new());
        self.config = config;

        Ok(())
    }

    /// Recompute the visible window from all_results[scroll_offset..]
    /// and set the cursor to `cursor_index` within the visible window.
    fn update_visible_results(&mut self, cursor_index: usize) {
        let max = self.config.max_results;
        let end = (self.scroll_offset + max).min(self.all_results.len());
        let visible: Vec<Symbol> = self.all_results[self.scroll_offset..end].to_vec();
        self.results = SelectableVec::new(visible);
        if !self.results.is_empty() {
            self.results.set_selected_index(cursor_index).ok();
        }
    }
}

impl fmt::Display for SymbolJumpMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SYMBOL")
    }
}

impl SearchSelectMode for SymbolJumpMode {
    type Item = Symbol;

    fn search(&mut self) {
        // Filter symbols into all_results (no truncation)
        if self.input.is_empty() {
            self.all_results = self.symbols.clone();
        } else {
            self.all_results = fragment::matching::find(&self.input, &self.symbols, usize::MAX)
                .into_iter()
                .map(|i| i.clone())
                .collect();
        }

        self.scroll_offset = 0;
        self.update_visible_results(0);
    }

    fn query(&mut self) -> &mut String {
        &mut self.input
    }

    fn insert_mode(&self) -> bool {
        self.insert
    }

    fn set_insert_mode(&mut self, insert_mode: bool) {
        self.insert = insert_mode;
    }

    fn results(&self) -> Iter<'_, Symbol> {
        self.results.iter()
    }

    fn selection(&self) -> Option<&Symbol> {
        self.results.selection()
    }

    fn selected_index(&self) -> usize {
        self.results.selected_index()
    }

    fn select_previous(&mut self) {
        if self.results.selected_index() == 0 {
            // At top of visible window — scroll up if possible
            if self.scroll_offset > 0 {
                self.scroll_offset -= 1;
                self.update_visible_results(0);
            }
        } else {
            self.results.select_previous();
        }
    }

    fn select_next(&mut self) {
        let visible_len = self.results.len();
        if visible_len == 0 {
            return;
        }
        if self.results.selected_index() >= visible_len - 1 {
            // At bottom of visible window — scroll down if possible
            if self.scroll_offset + self.config.max_results < self.all_results.len() {
                self.scroll_offset += 1;
                self.update_visible_results(visible_len - 1);
            }
        } else {
            self.results.select_next();
        }
    }

    fn config(&self) -> &SearchSelectConfig {
        &self.config
    }
}

fn symbols<'a, T>(tokens: T) -> Vec<Symbol>
where
    T: Iterator<Item = Token<'a>>,
{
    let eligible_scopes =
        ScopeSelectors::from_str("entity.name.function, entity.name.class, entity.name.struct")
            .unwrap();
    tokens
        .filter_map(|token| {
            if let Token::Lexeme(lexeme) = token {
                // Build a symbol, provided it's of the right type.
                if eligible_scopes
                    .does_match(lexeme.scope.as_slice())
                    .is_some()
                {
                    return Some(Symbol {
                        token: lexeme.value.to_string(),
                        position: lexeme.position,
                    });
                }
            }

            None
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::SymbolJumpMode;
    use super::{symbols, Symbol};
    use crate::models::application::modes::{SearchSelectConfig, SearchSelectMode};
    use crate::models::application::Application;
    use scribe::buffer::{Lexeme, Position, ScopeStack, Token};
    use std::path::Path;
    use std::str::FromStr;

    #[test]
    fn symbols_are_limited_to_functions() {
        let tokens = vec![
            Token::Lexeme(Lexeme {
                value: "text",
                position: Position { line: 0, offset: 0 },
                scope: ScopeStack::from_str("meta.block.rust").unwrap(),
            }),
            Token::Lexeme(Lexeme {
                value: "function",
                position: Position { line: 1, offset: 0 },
                scope: ScopeStack::from_str("entity.name.function").unwrap(),
            }),
            Token::Lexeme(Lexeme {
                value: "non-function",
                position: Position { line: 2, offset: 0 },
                scope: ScopeStack::from_str("meta.entity.name.function").unwrap(),
            }),
        ];

        let results = symbols(tokens.into_iter());
        assert_eq!(results.len(), 1);
        assert_eq!(
            results.first().unwrap(),
            &Symbol {
                token: "function".to_string(),
                position: Position { line: 1, offset: 0 }
            }
        );
    }

    #[test]
    fn reset_clears_query_mode_and_results() {
        let config = SearchSelectConfig::default();
        let mut mode = SymbolJumpMode::new(config.clone()).unwrap();
        let mut app = Application::new(&[]).unwrap();

        // Open this file so the test can search for its own function symbol.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file!());
        app.workspace.open_buffer(&path).unwrap();
        let token_set = app.workspace.current_buffer_tokens().unwrap();

        // Do an initial reset to get the results populated
        mode.reset(&token_set, config.clone()).unwrap();
        mode.query().push_str("reset_clears_query_mode_and_results");
        mode.set_insert_mode(false);
        mode.search();

        // Ensure we have results before reset
        assert!(mode.results.len() > 0);

        mode.reset(&token_set, config).unwrap();
        assert_eq!(mode.query(), "");
        assert_eq!(mode.insert_mode(), true);
        assert_eq!(mode.results().len(), 0);
    }

    #[test]
    fn scrolling_through_results() {
        let config = SearchSelectConfig { max_results: 2 };
        let mut mode = SymbolJumpMode::new(config).unwrap();
        let mut app = Application::new(&[]).unwrap();

        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file!());
        app.workspace.open_buffer(&path).unwrap();
        let token_set = app.workspace.current_buffer_tokens().unwrap();

        mode.reset(&token_set, SearchSelectConfig { max_results: 2 })
            .unwrap();
        mode.search();

        // Should have all results in all_results, only 2 visible
        let total = mode.all_results.len();
        if total > 2 {
            // Can scroll down
            mode.select_next();
            mode.select_next(); // hit bottom of visible window, should scroll
            assert_eq!(mode.scroll_offset, 1);
            // Can scroll back up
            mode.select_previous(); // hit top of visible window, should scroll
            assert_eq!(mode.scroll_offset, 0);
        }
    }
}
