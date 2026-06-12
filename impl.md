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


which-key

| File | Change |
|------|--------|
| `input/mod.rs` | Added `Key::display()` for human-readable key labels |
| `input/key_map/mod.rs` | Added `REVERSE_COMMAND_MAP` (LazyLock), `format_command_name`, `is_housekeeping_command`, `KeyMap::which_key_entries`, `KeyMap::which_key_leader_entries` |
| `view/presenter.rs` | Added `print_which_key_popup(title, entries)` — renders a bottom-right bordered popup with key→description rows, min 30 chars wide, max 10 rows |
| `models/application/mod.rs` | Changed `Mode::PendingLeader(_)` to `Mode::PendingLeader(ref mut mode)` in `present()` to pass mode data to presenter |
| `presenters/modes/pending_leader.rs` | Updated signature to accept `&PendingLeaderMode`, shows leader tree entries filtered by pressed keys, title shows pressed sequence |
| `presenters/modes/pending_delete.rs` | Shows which-key popup with `pending_delete` mode bindings |
| `presenters/modes/pending_yank.rs` | Shows which-key popup with `pending_yank` mode bindings |
| `presenters/modes/pending_change.rs` | Shows which-key popup with `pending_change` mode bindings |
| `presenters/modes/pending_left_bracket.rs` | Shows which-key popup with `pending_left_bracket` mode bindings |
| `presenters/modes/pending_right_bracket.rs` | Shows which-key popup with `pending_right_bracket` mode bindings |

The which-key popup:

Appears in the bottom-right corner above the status line
Is minimum 30 characters wide, auto-sizing to content
Auto-filters based on pressed keys (leader mode traverses the tree, pending modes show only valid next keys)
Strips housekeeping commands (switch_to_normal_mode, scroll_to_cursor, handle_input) from descriptions, showing only the meaningful action
Formats command names by stripping module prefix and converting snake_case → Title Case
Shows Esc → Cancel specially, and subtree nodes as key → …
Uses the same visual style as existing popups (dark background, Unicode box drawing, accent-colored keys and titles)

Add pending_g: session
```
pending_g:
  g: cursor::move_to_first_line
  l: application::switch_to_line_jump_mode
  h: cursor::move_to_previous_hunk
  n: cursor::move_to_next_hunk
  r: git::revert_hunk
  d: git::show_hunk_diff
  t: tag::tag_under_cursor
  escape: application::switch_to_normal_mode
  _:
    - application::switch_to_normal_mode
    - application::handle_input
```

Summary of Implemented Feature: :gs (Git Status Buffer)
Core Concept
A virtual buffer (like rg and sed) that displays git status, branches, and stash — with interactive keybindings to stage/unstage files and open them.

src/models/application/buffer_metadata.rs	Added GitStatus variant to BufferType enum + is_git_status() method on BufferRegistry
src/commands/git_status.rs	New file — all git status logic
src/commands/mod.rs	Added pub mod git_status; + registered 4 commands in hash_map()
src/commands/application.rs	Added key interception block for git status buffer in handle_input() + updated open_under_cursor() dispatch
src/commands/ex.rs	Added "gs" match arm to accept_input()

What :gs Shows
The buffer displays:

Current branch name
Staged files (from git status --short, index column)
Unstaged files (worktree column)
Untracked files (?? entries)
"Nothing to commit" message when clean
Top 5 branches (sorted by most recent committer date)
Key Bindings (active only in git status buffer)
Key
Action
Description
s	git_status::stage_file	In Staged section → git reset HEAD -- <path> (unstage). In Unstaged/Untracked → git add <path> (stage). Auto-refreshes buffer afterward, repositions cursor on same file
z	git_status::show_stash	Runs git stash list, shows up to 10 entries as a popup overlay
enter	git_status::open_under_cursor	Parses the file path from the current line, opens it in the editor
q	buffer::close	Closes the git status buffer
r	git_status::show	Refreshes/re-runs git status
j/k	(falls through to normal keymap)	Cursor navigation

Buffer is created with virtual path [Git Status] and registered as BufferType::GitStatus
In handle_input(), before the normal keymap lookup, we check is_git_status(buf.id)
If true, intercept s, z, enter, q, r keys; all other keys fall through to normal mode bindings
The open_under_cursor() dispatch in application.rs was also updated so that pressing enter in normal mode (which normally triggers switch_to_symbol_jump_mode) routes to git_status::open_under_cursor when the buffer path is [Git Status], matching the existing pattern for [Ripgrep Results].
