# Rivet — Architecture

_Last updated: Phase 0 (scaffolding)_

---

## Guiding Principles

1. **Correctness over cleverness.** Prefer simple, auditable code.
2. **Unsafe is quarantined.** The compiler-enforced `#![deny(unsafe_code)]` at
   the crate root means unsafe can only appear in the two designated modules.
   Anything that touches Win32 or Scintilla FFI stays behind a safe abstraction
   boundary.
3. **One thread owns the UI.** All Win32 windowing, message dispatch, and
   Scintilla operations happen on the main thread. Background threads communicate
   back via `PostMessage` or channels — never by touching UI objects directly.
4. **No global mutable state.** Application state flows through explicit
   function arguments or a single top-level `App` struct passed by mutable
   reference.

---

## Planned Module Layout

```
rivet/
└── src/
    ├── main.rs               Entry point; WinMain equivalent (Phase 2)
    ├── app.rs                Application lifecycle & top-level state (Phase 2)
    │
    ├── platform/             Platform abstraction layer
    │   ├── mod.rs            Public safe traits; NO unsafe here
    │   └── win32/            ◄ unsafe allowed ►
    │       ├── mod.rs        Re-exports; module-level #![allow(unsafe_code)]
    │       ├── window.rs     Main window creation, WndProc, message loop
    │       ├── dialogs.rs    Common dialogs (Open / Save As / Find)
    │       └── dpi.rs        Per-monitor DPI v2 helpers
    │
    ├── editor/               Editor component abstraction
    │   ├── mod.rs            Safe editor API (document model, view commands)
    │   └── scintilla/        ◄ unsafe allowed ►
    │       ├── mod.rs        Scintilla child-window lifecycle
    │       └── messages.rs   Type-safe wrappers around SCI_* messages
    │
    ├── ui/                   High-level UI components (safe Rust)
    │   ├── mod.rs
    │   ├── tabs.rs           Tab bar state and rendering
    │   └── statusbar.rs      Status bar (encoding, EOL, caret position)
    │
    ├── session/              Session persistence (open files, caret, prefs)
    │   └── mod.rs
    │
    ├── search/               Find / Replace / Find-in-Files
    │   └── mod.rs
    │
    └── config/               User configuration (key bindings, themes, prefs)
        └── mod.rs
```

Modules not yet implemented are listed here to establish the intended
boundaries before any code is written.

---

## Threading Model

```
┌─────────────────────────────────────────────────────────┐
│  Main thread (UI thread)                                │
│                                                         │
│  WinMain → message loop → WndProc dispatch              │
│  All Win32 calls, Scintilla messages, UI state writes   │
└────────────────────┬────────────────────────────────────┘
                     │  PostMessage / channel send
          ┌──────────┴──────────┐
          │                     │
  ┌───────▼──────┐    ┌─────────▼────────┐
  │ Search worker│    │ File I/O worker   │
  │ (find-in-    │    │ (large file load, │
  │  files)      │    │  encoding detect) │
  └──────────────┘    └──────────────────┘
```

Rules:
- Worker threads **never** call Win32 UI functions or send Scintilla messages.
- Results are marshalled back to the main thread via `PostMessage(hwnd, …)` or
  a `std::sync::mpsc` channel drained in the message loop.
- Workers are joined (or cancelled via an `AtomicBool` flag) before the
  application exits to avoid dangling threads.

---

## Unsafe Policy

| Module | `unsafe` permitted | Rationale |
|--------|--------------------|-----------|
| `platform::win32` | Yes | Win32 API is inherently unsafe C FFI |
| `editor::scintilla` | Yes | Scintilla hosted as a C++ child window |
| Everything else | **No** (`#![deny(unsafe_code)]` at crate root) | |

Every `unsafe` block **must** have a `// SAFETY:` comment that states:

- Which invariant makes the operation safe.
- What the caller is responsible for maintaining.

Example:

```rust
// SAFETY: `hwnd` was returned by CreateWindowExW and has not been destroyed.
// The Scintilla control is alive for the lifetime of `self`.
unsafe { SendMessageW(hwnd, SCI_SETTEXT, 0, text_ptr as LPARAM) };
```

---

## Error Handling

- A central `rivet::Error` enum (defined in Phase 2) covers all error
  categories: `Io`, `Encoding`, `Win32(HRESULT)`, `ScintillaMsg`.
- User-visible errors are surfaced through Win32 `MessageBoxW` dialogs, not
  panics.
- `unwrap()` / `expect()` are forbidden in production paths; they are only
  acceptable in `#[test]` code or with a clearly documented panic-safety
  argument.

---

## Performance Targets

| Operation | Target |
|-----------|--------|
| Cold startup (empty session) | < 150 ms wall time |
| Open 10 MB text file | < 500 ms |
| Open 100 MB file (Large File Mode) | < 2 s |
| Find-next (in document) | < 16 ms |
| Find-in-files (10k files) | responsive with cancel within 200 ms |

These targets will be tracked with a simple timestamp harness (Phase 2) and
later a criterion benchmark (Phase 6+).

---

## Large File Mode

Files above **50 MB** automatically enter Large File Mode:

- Word wrap disabled.
- Full syntax highlighting disabled (lexer set to plain-text).
- Status bar shows "Large File Mode" indicator.
- Session checkpoint skips content saving (metadata only).

Threshold is a compile-time constant (`editor::LARGE_FILE_THRESHOLD_BYTES`)
to make it easy to tune.

---

## Session Persistence

Session state is stored as a JSON file in:

```
%APPDATA%\Rivet\session.json
```

Contents: list of open file paths + per-file state (caret position, scroll
offset, encoding, EOL mode, view settings). Missing files are silently skipped
on restore with a non-intrusive status-bar notice.

A periodic checkpoint (every N edits or T seconds) writes metadata only —
never file content — to reduce crash-induced state loss.

---

_This document is updated at the start of each phase to reflect the current
module boundaries and any architectural decisions made during implementation._
