# Phase 5 — Editing Essentials: Task Breakdown

_Written after Phase 4 (commit 7b5700e)._

---

## Scope

Phase 5 wires up the full standard editing command set, adds a per-document
word-wrap toggle, and adds EOL conversion.

| Sub-phase | What |
|-----------|------|
| 5a | Full Edit menu: Undo, Redo, Cut, Copy, Paste, Delete, Select All |
| 5b | Word Wrap toggle (View menu, per-document, checkmark) |
| 5c | Format menu: Convert EOL to CRLF / LF / CR |

---

## Architecture notes

### Scintilla clipboard messages

Scintilla processes the standard Win32 clipboard messages natively
(documented in ScintillaDoc.html §Clipboard):

| Operation | Message sent |
|-----------|-------------|
| Undo | `WM_UNDO` (0x0304) |
| Cut | `WM_CUT` (0x0300) |
| Copy | `WM_COPY` (0x0301) |
| Paste | `WM_PASTE` (0x0302) |
| Delete | `WM_CLEAR` (0x0303) |
| Redo | `SCI_REDO` (2179) — no Win32 equivalent |
| Select All | `SCI_SELECTALL` (2013) |

### Word wrap

Stored as `word_wrap: bool` in `DocumentState` (defaults to `false`).
`ScintillaView::set_word_wrap(bool)` sends `SCI_SETWRAPMODE` with
`SC_WRAP_WORD` (1) or `SC_WRAP_NONE` (0).

The View > Word Wrap menu checkmark is updated via `CheckMenuItem` with
`MF_BYCOMMAND` whenever:
- The user clicks View > Word Wrap
- The user switches tabs (`TCN_SELCHANGE`)
- The last tab is reset to untitled (close guard)

### EOL conversion

`SCI_CONVERTEOLS` (2029) converts all existing line endings in the document
to the given mode (`SC_EOL_CRLF / SC_EOL_LF / SC_EOL_CR`).  This modifies
the buffer, so Scintilla fires `SCN_SAVEPOINTLEFT` automatically — no manual
`dirty` flip required.  `SCI_SETEOLMODE` is also called so new keystrokes
use the converted style.

---

## Files changed

| File | What changed |
|------|-------------|
| `src/app.rs` | Added `word_wrap: bool` to `DocumentState` |
| `src/editor/scintilla/messages.rs` | Added `SCI_UNDO`, `SCI_REDO`, `SCI_SELECTALL`, `SCI_CONVERTEOLS`, `SCI_GETWRAPMODE`, `SC_WRAP_WORD`, `WM_CUT/COPY/PASTE/CLEAR/UNDO` |
| `src/editor/scintilla/mod.rs` | Added `undo`, `redo`, `cut`, `copy_to_clipboard`, `paste`, `delete_selection`, `select_all`, `convert_eols`, `set_word_wrap`, `is_word_wrap` |
| `src/platform/win32/window.rs` | New IDM constants; full Edit menu; Format menu; View > Word Wrap live; `handle_eol_convert`, `handle_word_wrap_toggle`, `update_wrap_checkmark`; removed `MF_GRAYED` from Save/Save As |
| `docs/phase5-tasks.md` | This file |

---

## New menu layout

```
File | Edit | Format | View | Help

File
  New           Ctrl+N
  ──────
  Open…         Ctrl+O
  Save          Ctrl+S
  Save As…
  ──────
  Close Tab     Ctrl+W
  ──────
  Exit          Alt+F4

Edit
  Undo          Ctrl+Z
  Redo          Ctrl+Y
  ──────
  Cut           Ctrl+X
  Copy          Ctrl+C
  Paste         Ctrl+V
  Delete
  ──────
  Select All    Ctrl+A

Format
  Convert to Windows (CRLF)
  Convert to Unix (LF)
  Convert to Classic Mac (CR)

View
  Word Wrap     (toggleable checkmark)

Help
  About Rivet…
```

---

## New accelerators (Phase 5 additions)

| Key | Command |
|-----|---------|
| Ctrl+Z | Undo |
| Ctrl+Y | Redo |
| Ctrl+X | Cut |
| Ctrl+C | Copy |
| Ctrl+V | Paste |
| Ctrl+A | Select All |

---

## How to test

1. **Edit menu**
   - Type some text; Ctrl+Z undoes; Ctrl+Y redoes
   - Select text; Ctrl+X cuts; Ctrl+V pastes it back
   - Ctrl+A selects all; Edit > Delete clears it
2. **Word Wrap**
   - Open a file with long lines; View > Word Wrap — lines wrap; checkmark appears
   - Open second tab; word wrap is independent (off by default)
   - Switch back — checkmark returns to the first tab's state
3. **EOL Conversion**
   - Open a CRLF file; Format > Convert to Unix (LF); status bar shows "LF"
   - Tab becomes dirty (`*` prefix); Ctrl+S saves; reopen — LF only

---

## Deferred to Phase 6+

| Item | Phase |
|------|-------|
| Go To Line (Ctrl+G) | 6 (dialog infrastructure) |
| Recent Files list | 6 |
| Persist untitled buffer content | 6 |
| Dynamic Undo/Redo enable/disable | 8 |
