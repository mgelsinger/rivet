# Phase 3 — File I/O Foundation: Task Breakdown

_Written after Phase 2b (commit d96e337) to clarify scope and split work
into PR-sized chunks._

---

## What Phase 2 actually delivered

| Sub-phase | Commit | Delivered |
|-----------|--------|-----------|
| 2a | 9992f8b | Win32 window class, WndProc skeleton, message loop, menu bar (stubs), `RivetError`, `show_error_dialog`, debug startup timer |
| 2b | d96e337 | `ScintillaView` (DLL load, child window, UTF-8 init, `Drop`), status bar child window (placeholder text), `WM_SIZE` layout |

### What Phase 2 missed (now remediated)

`ARCHITECTURE.md` listed `app.rs` (Application lifecycle & top-level state)
as a Phase 2 deliverable. It was created retroactively with a minimal skeleton
(`App::new()`, `App::window_title()`) and wired into `WindowState`.
`RivetError::Encoding` and `RivetError::ScintillaMsg` variants were also added
to complete the error enum specified in ARCHITECTURE.md.

---

## Phase 3 sub-tasks

### Phase 3a — Document state model

**Goal:** Expand `App` / introduce `DocumentState` that all subsequent
sub-phases depend on. No visible user-facing change.

> `src/app.rs` skeleton and `WindowState::app` field already exist (Phase 2
> remediation).  This sub-phase adds the document-state detail.

Files to modify:
- `src/app.rs` — add `DocumentState`; embed it in `App`
- `src/platform/win32/window.rs` — no structural change needed

Deliverables:
- `DocumentState` struct fields:
  - `path: Option<PathBuf>` — `None` = untitled
  - `encoding: Encoding` (enum: `Utf8`, `Utf16Le`, `Utf16Be`, `Ansi`)
  - `eol: EolMode` (enum: `Crlf`, `Lf`, `Cr`)
  - `dirty: bool`
  - `large_file: bool` — set when file exceeds `LARGE_FILE_THRESHOLD_BYTES`
- `App::new()` returns a default (untitled, UTF-8, CRLF, clean) state
- `WindowState` gains an `app: App` field, initialized in `WM_CREATE`
- Helper: `App::window_title() -> String` → `"Rivet"` / `"filename — Rivet"` /
  `"*filename — Rivet"`

How to test:
- Build succeeds; window opens and behaves identically to Phase 2b
- `App::window_title()` unit-tested for the three title variants

Unsafe notes: none — this is pure safe Rust.

---

### Phase 3b — File > Open (dialog + read + load into Scintilla)

**Goal:** User can open a file from disk and see it in the editor.

Files to create / modify:
- `src/platform/win32/dialogs.rs` (new) — `show_open_dialog(hwnd) -> Option<PathBuf>`
- `src/platform/win32/mod.rs` — uncomment `pub mod dialogs;`
- `src/platform/win32/window.rs` — handle `IDM_FILE_OPEN` in `WM_COMMAND`
- `src/editor/scintilla/messages.rs` — add `SCI_SETTEXT`, `SCI_CLEARALL`,
  `SCI_SETSAVEPOINT`, `SCI_SETLEXER` (SCLEX_NULL for large-file mode)
- `src/editor/scintilla/mod.rs` — add `ScintillaView::load_text(bytes: &[u8])`
- `src/app.rs` — `App::open_file(path, bytes)` — detects encoding, sets state
- `Cargo.toml` — add `Win32_UI_Controls_Dialogs` (or `Win32_UI_Shell`) feature
  for `GetOpenFileNameW`

Deliverables:
- Menu: `File > Open…` (`Ctrl+O`) is enabled and opens a standard file picker
- Encoding detection (in order):
  1. UTF-16 LE BOM (`FF FE`)
  2. UTF-16 BE BOM (`FE FF`)
  3. UTF-8 BOM (`EF BB BF`)
  4. Heuristic: valid UTF-8 → `Encoding::Utf8`
  5. Fallback: `Encoding::Ansi` (load bytes as-is; Scintilla treats as Latin-1)
- Content loaded into Scintilla via `SCI_SETTEXT` (UTF-8 only; transcode
  UTF-16 → UTF-8 before passing)
- Large File Mode (>50 MiB): `SCI_SETLEXER(SCLEX_NULL)`, word-wrap off,
  status bar suffix "  [Large File]"
- Window title updated: `"filename — Rivet"`
- `DocumentState` updated with path, encoding, large_file flag

How to test:
- Open a UTF-8 file → content visible, title shows filename
- Open a UTF-16 LE file → content visible, `encoding` field = `Utf16Le`
- Open a >50 MiB file → Large File Mode indicator in status bar
- Cancel dialog → no change

Unsafe notes:
- `GetOpenFileNameW` call in `dialogs.rs` (unsafe block required)
- `SCI_SETTEXT` sends a pointer as LPARAM — existing pattern from 2b

---

### Phase 3c — File Save + Save As + dirty tracking

**Goal:** User can save the current document; unsaved changes are indicated
in the title and guarded on close.

Files to create / modify:
- `src/platform/win32/dialogs.rs` — add `show_save_dialog(hwnd, default_name) -> Option<PathBuf>`
- `src/platform/win32/window.rs`:
  - Handle `IDM_FILE_SAVE`, `IDM_FILE_SAVE_AS` in `WM_COMMAND`
  - Handle `WM_NOTIFY` → `SCN_SAVEPOINTLEFT` / `SCN_SAVEPOINTREACHED`
  - Guard `WM_CLOSE` with a "Save changes?" `MessageBoxW` when `dirty == true`
- `src/editor/scintilla/messages.rs` — add `SCN_SAVEPOINTLEFT`,
  `SCN_SAVEPOINTREACHED`, `SCI_SETSAVEPOINT`
- `src/app.rs` — `App::save(path, view) -> Result<()>` writes UTF-8 bytes
  (or transcodes to original encoding) to disk; calls `SCI_SETSAVEPOINT`
- `src/platform/win32/window.rs` — `WM_SETTEXT` to update window title on
  dirty change

Menu items to add:
- `File > Open…`  Ctrl+O  (`IDM_FILE_OPEN` — from 3b)
- `File > Save`   Ctrl+S  (`IDM_FILE_SAVE`)
- `File > Save As…`       (`IDM_FILE_SAVE_AS`)
- Separator before Exit

Deliverables:
- `Ctrl+S` saves to the current path; opens Save As dialog if untitled
- `File > Save As…` always opens the dialog
- Title shows `*` prefix when document is dirty; clears on save
- Closing with unsaved changes → Yes/No/Cancel dialog; Cancel aborts close
- Saving an ANSI or UTF-16 file round-trips the encoding (no forced UTF-8
  conversion of existing files)

How to test:
- Open a file, edit it → title gains `*`
- Save → `*` removed, file updated on disk
- Close with unsaved → prompt appears; Cancel keeps window open
- Save As → saves to chosen path; title updates

Unsafe notes:
- `GetSaveFileNameW` call (same pattern as `GetOpenFileNameW`)
- `WM_NOTIFY` reading `NMHDR*` from LPARAM (existing Win32 pattern)

---

### Phase 3d — Live status bar

**Goal:** Status bar reflects actual document state (encoding, EOL, caret
position) instead of the placeholder set in Phase 2b.

Files to create / modify:
- `src/editor/scintilla/messages.rs` — add:
  - `SCI_GETCURRENTPOS`, `SCI_LINEFROMPOSITION`, `SCI_GETCOLUMN`
  - `SCI_GETEOLMODE` → `SC_EOL_CRLF`, `SC_EOL_LF`, `SC_EOL_CR`
  - `SCN_UPDATEUI`
- `src/editor/scintilla/mod.rs` — add `ScintillaView::caret_line_col()`,
  `ScintillaView::eol_mode()`
- `src/platform/win32/window.rs`:
  - Split status bar into parts: `SB_SETPARTS` with widths
    (encoding | EOL | caret position)
  - Handle `WM_NOTIFY` → `SCN_UPDATEUI`: query caret + EOL, call
    `update_status_bar(state)`
  - `update_status_bar(state: &WindowState)` — assembles the three segment
    strings and calls `SB_SETTEXT` for each part

Status bar format (three parts):
- Part 0: `UTF-8` / `UTF-16 LE` / `UTF-16 BE` / `ANSI`
- Part 1: `CRLF` / `LF` / `CR`
- Part 2: `Ln 1, Col 1` (1-based; column is character column, not byte offset)

How to test:
- Open a file, click around → Ln/Col updates correctly
- Open a CRLF file → Part 1 shows `CRLF`
- Open a UTF-16 LE file → Part 0 shows `UTF-16 LE`

Unsafe notes:
- `SB_SETPARTS` / `SB_SETTEXT` are existing patterns from 2b
- `SCN_UPDATEUI` LPARAM cast to `SCNotification*` is the standard Scintilla
  notification pattern (requires a `// SAFETY:` comment)

---

## Dependency order

```
3a (App struct)
  └── 3b (File Open)
        └── 3c (File Save + dirty)
              └── 3d (Live status bar)
```

Each sub-phase is a standalone PR.  `3c` can be started before `3b` is
merged (the save dialog does not depend on the open dialog's implementation),
but `3a` must land first.

---

## Cargo features to add in Phase 3

| Feature | Sub-phase | Used for |
|---------|-----------|----------|
| `Win32_UI_Shell` or `Win32_UI_Dialogs` | 3b | `GetOpenFileNameW` |
| (same) | 3c | `GetSaveFileNameW` |

Confirm exact feature name against `windows` crate 0.58 docs before adding.

---

## Deferred to later phases

| Item | Phase |
|------|-------|
| New file (Ctrl+N) | 4 (Tabs) |
| Recent files list | 4 (Tabs) |
| Encoding conversion on save | 5 (Editing essentials) |
| EOL conversion on save | 5 |
| `File > Reload` | 5 |
| DPI-aware status bar part widths | 8 |
| `LoadLibraryExW` with full path (DLL hardening) | 10 |
