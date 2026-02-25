# Phase 6 — Find / Replace + Go To Line: Task Breakdown

_Written after Phase 5 (commit 7d2a72e)._

---

## Scope

Phase 6 wires up three essential navigation features:

| Sub-phase | Feature |
|-----------|---------|
| 6a | Find dialog (Ctrl+F) + F3 / Shift+F3 repeat |
| 6b | Replace dialog (Ctrl+H) + Replace / Replace All |
| 6c | Go To Line dialog (Ctrl+G) |

"Find in Files" (cross-directory search with a results panel) requires a worker
thread and is deferred to a later phase.

---

## Architecture notes

### Win32 common Find/Replace dialogs

Windows provides modeless Find and Replace dialogs via `FindTextW` /
`ReplaceTextW` (`Win32_UI_Controls_Dialogs`, already enabled).  These stay open
while the user keeps editing and send the registered `"commdlg_FindReplace"`
message to the owner window whenever the user clicks Find Next / Replace /
Replace All / closes.

The message ID is registered once in `run()` via `RegisterWindowMessageW` and
stored in a `OnceLock<u32>`.  `wnd_proc` checks it before the standard `match
msg` so the custom message is always intercepted.

`IsDialogMessageW` is called in `message_loop` before `TranslateAcceleratorW`
so that Tab, Enter, Escape, and arrow keys are routed to the modeless dialog
when it is open.

### Go To Line

No OS-provided dialog exists.  A minimal modal dialog is built at runtime by
encoding a `DLGTEMPLATE` + `DLGITEMTEMPLATE` entries as a `Vec<u8>` and
passing it to `DialogBoxIndirectParamW`.  No `.rc` file is needed.

The template contains 4 controls:

| ID | Type | Purpose |
|----|------|---------|
| 0xFFFF | Static | "Go to line (1–N):" label |
| 100 | Edit | line-number input |
| 1 (IDOK) | Button (BS_DEFPUSHBUTTON) | OK |
| 2 (IDCANCEL) | Button | Cancel |

`WM_INITDIALOG` pre-fills the edit with the current line number and selects
all so the user can type immediately.  `WM_COMMAND / IDOK` validates the input
(1 ≤ n ≤ total) and calls `EndDialog(hwnd, n)`.

### Scintilla search API

Search uses the target range API:

1. Set search flags: `SCI_SETSEARCHFLAGS(SCFIND_MATCHCASE | SCFIND_WHOLEWORD)`
2. Set range: `SCI_SETTARGETSTART`, `SCI_SETTARGETEND`
   - If start > end → backward search
3. Search: `SCI_SEARCHINTARGET(len, text)` → match start position or -1
4. After match: `SCI_GETTARGETEND` → match end position
5. Select: `SCI_SETSEL(match_start, match_end)` (also scrolls into view)

`replace_all` wraps all replacements in `SCI_BEGINUNDOACTION` /
`SCI_ENDUNDOACTION` so the entire operation is a single Ctrl+Z step.

### FINDREPLACEW lifetime

`find_buf` and `replace_buf` are `Box<[u16; 512]>` in `WindowState`.
Their heap addresses are stored in `FINDREPLACEW.lpstrFindWhat` /
`lpstrReplaceWith` at creation time.  Since `WindowState` is stored as a raw
pointer in `GWLP_USERDATA` and never moved, all internal pointers remain valid
for the lifetime of the window.

---

## Files changed

| File | What changed |
|------|-------------|
| `src/main.rs` | Added `mod search;` |
| `src/search/mod.rs` | **Created** — `SearchOptions` struct |
| `src/editor/scintilla/messages.rs` | Added `SCI_SETSEARCHFLAGS`, `SCI_SETTARGETSTART/END`, `SCI_GETTARGETSTART/END`, `SCI_SEARCHINTARGET`, `SCI_REPLACETARGET`, `SCI_GETSELECTIONSTART/END`, `SCI_SETSEL`, `SCI_SCROLLCARET`, `SCI_BEGINUNDOACTION`, `SCI_ENDUNDOACTION`, `SCI_GETLINECOUNT`, `SCI_POSITIONFROMLINE`, `SCFIND_MATCHCASE`, `SCFIND_WHOLEWORD` |
| `src/editor/scintilla/mod.rs` | Added `doc_len`, `set_target`, `search_in_target`, `get_target_end`, `replace_target`, `selection_start`, `selection_end`, `set_sel`, `scroll_caret`, `begin_undo_action`, `end_undo_action`, `line_count`, `position_from_line`, `find_next`, `replace_all` |
| `src/platform/win32/window.rs` | New imports; IDM_SEARCH_* constants; FR_* constants; VK_F3; `FIND_MSG_ID` OnceLock; WindowState fields; `create_child_controls` init; Search menu; F3/Shift+F3 accelerators; `IsDialogMessageW` in message loop; FINDMSGSTRING registration in `run()`; `wnd_proc` FIND_MSG_ID check and IDM_SEARCH_* handlers; `handle_find_open`, `handle_replace_open`, `handle_findreplace_msg`, `handle_replace_once`, `handle_find_next`, `handle_goto_line`, `show_goto_line_dialog`, `goto_dlg_proc`, `build_goto_line_template`, `push_u16`, `push_u32`, `push_wstr`, `align4`, `pwstr_to_utf8` |
| `docs/phase6-tasks.md` | This file |

---

## New menu layout

```
File | Edit | Format | Search | View | Help

Search
  Find…          Ctrl+F    IDM_SEARCH_FIND
  Replace…       Ctrl+H    IDM_SEARCH_REPLACE
  Find Next      F3        IDM_SEARCH_FIND_NEXT
  Find Prev      Shift+F3  IDM_SEARCH_FIND_PREV
  ──────
  Go to Line…    Ctrl+G    IDM_SEARCH_GOTO_LINE
```

---

## How to test

**Find**
1. Ctrl+F — Find dialog opens.
2. Type text, click "Find Next" → match highlighted; dialog stays open.
3. F3 repeats forward; Shift+F3 repeats backward.
4. Search wraps around (bottom→top and vice versa).
5. No match → system beep (`MessageBeep`).
6. Close dialog → F3 still works from last search text.

**Replace**
1. Ctrl+H — Replace dialog opens.
2. "Replace" — replaces current match and advances to next.
3. "Replace All" — replaces every occurrence; count shown in a message box.
4. Replace All is a single undo step (Ctrl+Z reverts all replacements at once).

**Go To Line**
1. Ctrl+G — modal dialog opens, pre-filled with current line number.
2. Enter a valid line number → caret jumps to that line.
3. Out-of-range input → beep, dialog stays open.
4. Cancel → no change.

**Regression**
- Ctrl+N / Ctrl+O / Ctrl+S / Ctrl+W work.
- Edit menu (Undo/Redo/Cut/Copy/Paste/Delete/Select All) works.
- Word Wrap toggle and EOL conversion work.
- `cargo clippy -- -D warnings` passes.
- `cargo test` passes.

---

## Deferred to Phase 7+

| Item | Phase |
|------|-------|
| Dynamic Undo/Redo enable/disable | 8 |
| Persist last search flags across sessions | 7 |
| Find in Files (worker thread + results panel) | 9 |
| Regex search | 9 |
