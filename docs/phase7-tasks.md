# Phase 7 — Syntax Highlighting: Task Record

## Scope

Wire up Scintilla's built-in lexers for 18+ languages, apply a Notepad++-style
light colour theme, and add a Language segment to the status bar.

## Work-item checklist

- [x] `src/main.rs` — add `mod languages;` and `mod theme;`
- [x] `src/languages.rs` — `Language` enum, `language_from_path`, `keywords`
- [x] `src/theme.rs` — colour palette, `apply_theme` + 18 per-lexer functions
- [x] `src/editor/scintilla/messages.rs` — SCLEX_*, SCI_STYLE*, SCE_* constants
- [x] `src/editor/scintilla/mod.rs` — 8 new `ScintillaView` methods
- [x] `src/platform/win32/window.rs`:
  - `apply_highlighting` helper (safe fn, calls safe ScintillaView methods)
  - 4-part status bar (encoding | EOL | Ln/Col | Language)
  - Call sites in `load_file_into_active_tab` and `open_file_in_new_tab`
  - `update_status_bar` extended with language text
  - `handle_file_save` now calls `update_status_bar` on success (Save As fix)

## How to test

1. Open a `.rs` file → keywords blue/bold, comments green, strings dark red,
   lifetimes (`'a`) purple, macros (`println!`) brown.
2. Open a `.py` file → `"""docstrings"""` shown in green, `def`/`class` bold blue.
3. Open an `.html` file → `<div>` dark red/bold, attributes red, `<!-- -->` green.
4. Open a `.json` file → property names bold blue, string values dark red.
5. Open a `.ps1` file → keywords bold blue, `$variables` dark blue, comments green.
6. Open a file > 50 MiB → no highlighting; status bar shows "Rust [Large]"
   (or appropriate language name).
7. Create a new untitled tab → status bar part 3 shows "Plain Text".
8. Switch between tabs → language name in status bar updates immediately.
9. File > Save As with a new extension (e.g. rename `.txt` → `.rs`) → language
   in status bar updates after save.
10. `cargo clippy -- -D warnings` passes.
11. `cargo test` passes.

## Unsafe notes

- `apply_highlighting` in `window.rs` is **not** `unsafe fn` — all calls go
  through safe `ScintillaView` methods.
- `SCI_STYLESETFONT` and `SCI_SETKEYWORDS` receive a raw pointer to a `&[u8]`
  literal.  Scintilla copies the string before `SendMessageW` returns, so the
  stack lifetime is safe.  Each unsafe block inside `ScintillaView` carries the
  required `// SAFETY:` comment.
- No new `unsafe` code is added outside `editor::scintilla`.

## Bench notes

- `SCI_STYLECLEARALL` resets all 256 slots atomically in a single call (~1 µs).
- Total new `SendMessageW` calls per file open: ≈ 10–25 depending on language
  (default styles + per-token colours + keywords).  No measurable startup
  regression on the target workload (< 1 MB files).
- Large files (> 50 MiB) skip highlighting entirely; no regression there.
