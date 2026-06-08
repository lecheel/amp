feat: mega‑merge – ex mode, buffer registry, git gutter, vim operators, ripgrep, fmt_save & more

- Ex mode (:)
  - Full command line with history, tab completion, and popup grid navigation
  - Standard commands: :w, :q, :e, :bn, :bp, :bd, :ls
  - Aliases: :wq, :q!, and workspace navigation
  - Command aliases and application::nop to disable default keybindings
  - Improved :e command with directory walking (`walk_into_directory`), recursive subdirectory completions, trailing slashes, and absolute/relative path resolution

- Buffer management
  - Buffer list (:ls) with auto‑cleanup on file selection
  - Automatic removal of orphaned special buffers
  - BufferRegistry to track virtual/readonly types – fixes false [+] status, prevents save/edit on special buffers
  - Most Recently Used (MRU) buffer list support (`MRUMode`) with timestamp tracking, filtering, and switching via `SearchSelectMode`
  - Persistent buffer cursor positions with LRU pruning (up to 100 entries) and automatic restore on startup/loading
  - Refactored buffer operations to inline deletion logic in `delete_token` and `change_rest_of_line` to avoid double group nesting

- Git integration
  - Git gutter with added/modified/deleted line indicators
  - Git hunk navigation via [h and ]h
  - Git hunk revert functionality (`revert_hunk`) utilizing LCS-based diffing to restore index states under a single undo group

- Vim operators (opt‑in)
  - Dormant pending_delete/yank/change sections in default keymap
  - Enables dd/dw/yy/cw sequences when user remaps d/y/c
  - copy_token, copy_rest_of_line, and change_current_line commands for operator support
  - Repeat last change functionality (`.`) tracking repeatable buffer changes (`RepeatableAction`), insert-mode keystrokes, and nested operation groups
  - Refined transitions, ensuring `view::scroll_to_cursor` triggers after yank operations and `application::switch_to_normal_mode` occurs post-sequence

- Editing & navigation
  - Page up / page down using viewport height
  - Pending bracket modes [[ and ]] for toggle comment
  - Alt key, F1‑F12, and additional Ctrl keys (ctrl‑, ctrl‑], ctrl‑^, ctrl‑_)

- Ripgrep search workflow
  - :rg with virtual buffer, syntax‑highlighted results (YAML definition)
  - :cn, :cp to jump between results, :last_rg to return
  - open_under_cursor resolves file paths from grouped headings with column jump
  - Scroll cursor to center when opening results

- Formatting & save
  - fmt_save – auto‑format using format_tool
  - Whitespace cleanup and trailing newline assurance
  - Decoupled formatting logic to application preferences, delegating formatting execution to the `format` command
  - Improved error capture (stdout + stderr) for format tools, skipping early lines of error output and trimming redundant failure prefixes

- UI polish
  - Multi‑line error overlays in modal boxes with title/body split, integrated with `fmt_save` output
  - Bracketed paste support with single undo group

- Input enhancements
  - Tab completion for ex mode commands and file paths
  - Ctrl‑N / Ctrl‑P to cycle completions
  - Unified buffer word completion (Insert & Ex modes) with multi-column popup, tracking minimum 3-character prefixes
  - Enhanced unified completion with italicized dim gray ghost text preview for active selections
  - Leader key sequence system (`PendingLeaderMode`) with recursive leader tree support (`KeyMap::merge` using the entry API and `drain`) and dedicated status line presenter


```
example config.yml
keymap:
  normal:
    ",": application::switch_to_pending_leader_mode 
    d: application::switch_to_pending_delete_mode
    y: application::switch_to_pending_yank_mode
    c: application::switch_to_pending_change_mode
    "[": application::switch_to_pending_left_bracket_mode
    "]": application::switch_to_pending_right_bracket_mode
    ":": application::switch_to_ex_mode 
    enter: application::open_under_cursor
    l: "cursor::move_to_next_hunk"
    L: "cursor::move_to_previous_hunk"
    G: "cursor::move_to_last_line"
    k: "rg::search_under_cursor"
    alt-=: "workspace::next_buffer"
    alt--: "workspace::prev_buffer"
    alt-q: "application::exit"
    alt-d: "buffer::delete_current_line"
    alt-e: "application::run_file_manager"
    alt-b: "application::switch_to_buffer_list_mode"
    alt-w: "buffer::fmt_save"
    alt-x: "buffer::close"
    ctrl-b: "application::switch_to_mru_mode"
    f3: "rg::next_result"
    f4: "git::revert_hunk"
    f8: "application::switch_to_symbol_jump_mode"
    f11: "rg::switch_to_last_rg"
    page_down: "view::page_down"
    page_up: "view::page_up"
  insert:
    f3: "rg::next_result"
    f4: "git::revert_hunk"
    f8: "application::switch_to_symbol_jump_mode"
    alt-d: "buffer::delete_current_line"
    alt-w: "buffer::fmt_save"
    alt-x: "buffer::close"
    alt-q: "application::exit"
    alt-/: "completion::complete_from_buffer"
    ctrl-b: "application::switch_to_mru_mode"
    page_down: "view::page_down"
    page_up: "view::page_up"
  
leader:
  g:
    r: "git::revert_hunk"

pending_delete:
  d:
    - buffer::delete_current_line
    - application::switch_to_normal_mode
  $:
    - buffer::delete_rest_of_line
    - application::switch_to_normal_mode
  w:
    - buffer::delete_token
    - application::switch_to_normal_mode
```