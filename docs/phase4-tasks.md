# Phase 4 — Tabs + Session: Task Breakdown

_Written after Phase 3 (commit 0c05952)._

---

## Architecture decision: parallel ownership

`App` holds `Vec<DocumentState>` (pure Rust, fully testable).
`WindowState` holds a parallel `Vec<ScintillaView>` indexed identically.

```
WindowState {
    app:         App,               // Vec<DocumentState> + active_idx
    sci_views:   Vec<ScintillaView>,// same length as app.tabs; sci_views[i] ↔ app.tabs[i]
    sci_dll:     SciDll,            // shared DLL handle (dropped last)
    hwnd_tab:    HWND,              // SysTabControl32
    hwnd_status: HWND,
}
```

`App` stays Win32-free (no HWND/HINSTANCE imports) so unit tests keep working.
The parallel-vec invariant (`app.tabs.len() == sci_views.len()`) is maintained
exclusively in `window.rs`, which is the only place that creates/destroys tabs.

Drop order: `app` (contains no Win32) → `sci_dll` (FreeLibrary).
`ScintillaView` HWNDs are destroyed by Windows before `WM_DESTROY` fires on the
parent, so the `ScintillaView` struct holds a dead HWND at drop time — no cleanup needed.

---

## Phase 4a — Data-model refactor (no visible UI change)

**Goal:** Introduce `Vec<DocumentState>` + `SciDll`, update all callers. Behaviour
is identical to Phase 3; this is a pure internal refactor.

Files to create:
- `src/ui/mod.rs` — stub: `// placeholder`
- `src/session/mod.rs` — stub: `// placeholder`

Files to modify:
- `src/editor/scintilla/mod.rs`
  - Add `pub(crate) struct SciDll(HMODULE)` with `load() -> Result<Self>` and `Drop` calling `FreeLibrary`
  - `ScintillaView`: remove `dll` field; `create()` takes `&SciDll` (proof DLL is loaded)
  - Remove `impl Drop for ScintillaView` (child HWNDs destroyed by Windows before WM_DESTROY)
  - Add `pub(crate) fn show(&self, visible: bool)` — `ShowWindow(SW_SHOW / SW_HIDE)`
- `src/app.rs`
  - `App { tabs: Vec<DocumentState>, active_idx: usize }`
  - `App::active_doc() -> &DocumentState`, `App::active_doc_mut() -> &mut DocumentState`
  - `App::open_file()` and `App::save()` operate on `self.active_doc_mut()`
  - `App::window_title()` delegates to `self.active_doc()`
  - Update unit tests to construct `App` with a `Vec`
- `src/platform/win32/window.rs`
  - `WindowState`: `sci: ScintillaView` → `sci_views: Vec<ScintillaView>`, add `sci_dll: SciDll`
  - `create_child_controls`: `SciDll::load()`, `ScintillaView::create(hwnd, inst, &dll)`, `App::new()`
  - All `state.sci.*` → `state.sci_views[state.app.active_idx].*`
- `src/main.rs` — add `mod ui;` and `mod session;`

Unsafe notes: `SciDll::load()` calls `LoadLibraryW`; `SciDll::Drop` calls `FreeLibrary`.

---

## Phase 4b — Tab bar + multi-tab open + New File

**Goal:** Visible `SysTabControl32` at top; File > Open creates a new tab; Ctrl+N
opens an untitled tab; clicking a tab switches content.

Win32 tab-control constants (define locally like `SB_SETTEXT`):
```
TCM_FIRST         = 0x1300
TCM_INSERTITEMW   = TCM_FIRST + 7    // 0x1307
TCM_DELETEITEM    = TCM_FIRST + 8    // 0x1308
TCM_GETCURSEL     = TCM_FIRST + 11   // 0x130B
TCM_SETCURSEL     = TCM_FIRST + 12   // 0x130C
TCM_SETITEMW      = TCM_FIRST + 61   // 0x133D
TCM_GETITEMRECT   = TCM_FIRST + 10   // 0x130A

TCN_SELCHANGE     = 0xFFFFFDD9_u32   // (-551i32) as u32
TCN_SELCHANGING   = 0xFFFFFDD8_u32

TCIF_TEXT         = 0x0001
TCS_MULTILINE     = 0x0200
```

TCITEMW struct (define manually; windows crate may not expose it at 0.58):
```rust
#[repr(C)]
struct TCITEMW {
    mask: u32, dwState: u32, dwStateMask: u32,
    pszText: *mut u16, cchTextMax: i32,
    iImage: i32, lParam: LPARAM,
}
```

Files to modify:
- `src/app.rs`
  - `App::push_untitled() -> usize` — pushes new `DocumentState::new_untitled()`, returns index
  - `App::close_tab(idx: usize)` — removes from `tabs`, adjusts `active_idx`
  - `App::tab_label(idx: usize) -> String` — `"*name"` or `"name"` for display
- `src/platform/win32/window.rs`
  - `WindowState` gains real `hwnd_tab: HWND`
  - `create_child_controls` creates `SysTabControl32`; initial tab inserted via `TCM_INSERTITEMW`
  - `layout_children`: three-zone geometry (tab bar ≈ 24 px | Scintilla | status bar)
    - `TCM_GETITEMRECT` to measure actual tab bar height
  - `WM_NOTIFY`: add `TCN_SELCHANGE` arm: hide old sci, update `active_idx`, show new sci, refresh title + status bar
  - `handle_file_open`: check for duplicate path first; if not duplicate, call `open_in_new_tab`
  - New helper `open_in_new_tab(hwnd, state, path, bytes)`: `ScintillaView::create()`, push `App::push_untitled()`, insert `TCM_INSERTITEMW`, `set_text()`, `set_save_point()`
  - New helper `handle_new_file(hwnd, state)`: same but no file content
  - `IDM_FILE_NEW = 1000`; add to menu + `Ctrl+N` accelerator
  - `sync_tab_label(state, idx)`: update tab text via `TCM_SETITEMW`
  - `update_tab_label` called from `SCN_SAVEPOINTLEFT`/`REACHED` to refresh `*` prefix

How to test:
- `Ctrl+N` → second tab appears; content is empty
- `File > Open` → new tab with file content; title updates
- Click tab 1 → content switches back; status bar updates
- Dirty `*` appears on correct tab label

---

## Phase 4c — Session persistence

**Goal:** Restore previous session on startup; save on exit.

Cargo.toml additions:
```toml
[dependencies.serde]
version = "1"
features = ["derive"]

[dependencies.serde_json]
version = "1"
```

`deny.toml`: add `"BSL-1.0"` to allowed list (transitive dep of `serde_json` via `ryu`).

Files to create:
- `src/session/mod.rs` — real implementation (replaces stub):
  - `SessionFile { version: u32, tabs: Vec<TabEntry>, active_tab: usize }`
  - `TabEntry { path: Option<String>, caret_pos: usize, scroll_line: usize, encoding: String, eol: String }`
  - `session_path() -> Option<PathBuf>` — `%APPDATA%\Rivet\session.json`
  - `save(entries, active) -> io::Result<()>` — `create_dir_all` + `serde_json::to_writer_pretty`
  - `load() -> Option<SessionFile>` — read + parse; returns `None` on any error

Files to modify:
- `src/editor/scintilla/messages.rs` — add:
  - `SCI_GOTOPOS = 2025`
  - `SCI_GETFIRSTVISIBLELINE = 2152`
  - `SCI_SETFIRSTVISIBLELINE = 2613`
- `src/editor/scintilla/mod.rs` — add:
  - `caret_pos() -> usize` — `SCI_GETCURRENTPOS` (raw byte position)
  - `set_caret_pos(pos: usize)` — `SCI_GOTOPOS`
  - `first_visible_line() -> usize` — `SCI_GETFIRSTVISIBLELINE`
  - `set_first_visible_line(line: usize)` — `SCI_SETFIRSTVISIBLELINE`
- `src/app.rs` — add:
  - `Encoding::from_str(s: &str) -> Option<Self>`
  - `EolMode::from_str(s: &str) -> Option<Self>`
- `src/platform/win32/window.rs`
  - `WM_DESTROY`: call `save_session(state)` before dropping
  - `run()` startup: after window visible, call `restore_session(hwnd, state)`
  - `save_session(state)`: build `Vec<TabEntry>` from all tabs, call `session::save()`
  - `restore_session(hwnd, state)`: call `session::load()`, for each `TabEntry` with a `Some(path)` that `Path::exists()`, call `open_in_new_tab()`; skip untitled + missing; set `active_idx` after all opens; silently ignore errors

How to test:
- Open two files, position carets, close; relaunch → both files open at correct positions
- Delete a file between sessions → skipped silently, surviving file opens
- Corrupt `session.json` → empty session (one untitled tab), no crash

---

## Phase 4d — Close tab (Ctrl+W) + combined close guard

**Goal:** Close individual tabs with Ctrl+W; closing window with multiple dirty tabs
shows one combined prompt.

Files to modify:
- `src/editor/scintilla/mod.rs`
  - `pub(crate) fn destroy(&self)` — `DestroyWindow(self.hwnd)` (used when closing a tab mid-session)
- `src/platform/win32/window.rs`
  - `IDM_FILE_CLOSE = 1004`; `File > Close Tab   Ctrl+W` menu item; `Ctrl+W` accelerator
  - `handle_close_tab(hwnd, state, idx)`:
    1. If `app.tabs[idx].dirty`: prompt — save? (Yes/No/Cancel); Yes → call save flow; No → discard; Cancel → abort
    2. Explicitly `sci_views[idx].destroy()` then remove from `sci_views`
    3. `TCM_DELETEITEM` to remove from tab bar
    4. `App::close_tab(idx)`
    5. If `app.tabs` now empty: call `handle_new_file()` (always keep ≥ 1 tab)
    6. Adjust `active_idx`; show the now-active sci; update title + status bar
  - `WM_CLOSE` updated: collect all dirty tab names; if any, show combined dialog:
    `"The following files have unsaved changes:\n  • name1\n  • name2\nDiscard all and exit?"`
    — Yes/Cancel (no per-tab prompting on exit)

How to test:
- `Ctrl+W` on dirty tab → save prompt; Cancel → tab stays open
- `Ctrl+W` on last tab → becomes fresh untitled, window stays open
- Close window with two dirty tabs → combined prompt lists both names

---

## Dependency order

```
4a (data model refactor — behaviour unchanged)
  └── 4b (tab bar + multi-tab open)
        └── 4c (session save/restore)
              └── 4d (close tab + Ctrl+W)
```

---

## Deferred to Phase 5+

| Item | Phase |
|------|-------|
| Recent Files list | 5 |
| Tab reorder (drag-and-drop) | 8 |
| Middle-click close | 8 |
| Tab context menu | 8 |
| Tab tooltips (full path) | 8 |
| Periodic session checkpoint | 9 |
| Persist untitled buffer content | 5 |
