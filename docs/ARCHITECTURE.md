# Rivet — Architecture

_Last updated: Phase 3 (File I/O foundation)_

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

Modules marked _(Phase N)_ are planned but not yet present in the source tree.
Their boundaries are established here before any code is written to avoid
architectural drift.

---

## Phase 1 Decisions

### Win32 bindings — `windows` crate v0.58

The [`windows`](https://crates.io/crates/windows) crate (maintained by
Microsoft) is used for all Win32 / WinRT FFI.  It provides:

- **Feature-gated compilation** — only the Win32 modules we request are
  compiled, keeping incremental build times reasonable.
- **Safe wrappers** where available (COM, HRESULT error propagation).
- **`windows-sys`** parity — raw `extern "system"` signatures are available
  as a fallback when the safe wrappers are insufficient.

Features enabled in `Cargo.toml` (add new features in the phase that first
references the corresponding type):

| Feature | First used |
|---------|-----------|
| `Win32_Foundation` | Phase 2 |
| `Win32_Graphics_Gdi` | Phase 2 |
| `Win32_System_LibraryLoader` | Phase 2 (Scintilla DLL load) |
| `Win32_UI_Controls` | Phase 2 (status bar) |
| `Win32_UI_Controls_Dialogs` | Phase 3 (open/save dialogs) |
| `Win32_UI_WindowsAndMessaging` | Phase 2 |

### Scintilla integration — DLL hosting

**Decision: `SciLexer.dll` (DLL-hosting approach).**

| Criterion | Static lib | DLL hosting |
|-----------|-----------|-------------|
| Build complexity | High (C++ via `cc`, needs `cl.exe`) | Low (no compilation) |
| Distribution | Single `.exe` | `rivet.exe` + `SciLexer.dll` |
| Abstraction boundary | Identical | Identical |
| Scintilla design intent | Possible | Idiomatic |

The safe abstraction in `editor::scintilla` is identical either way —
switching to a static link in Phase 10 (packaging) is a `build.rs` change
only, not a code change.

**Version target:** Scintilla 5.x (latest stable at time of Phase 2
integration; minimum 5.2 for `SCI_COUNTCHARACTERS`).

**Load sequence (Phase 2):**
1. `LoadLibraryW("SciLexer.dll")` from the directory of the running `.exe`.
2. Scintilla registers the `"Scintilla"` window class on load.
3. `CreateWindowExW` with class `"Scintilla"` creates the editor child window.
4. All editor operations are `SendMessageW(hwnd, SCI_*, wparam, lparam)`.

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
