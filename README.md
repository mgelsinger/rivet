# Rivet

A simple, fast, and correct text editor for Windows 10/11 x64.

> **Status:** v0.1.0 — feature-complete MVP

---

## Goals

| Priority | Goal |
|----------|------|
| 1 | **Correctness** — no data loss; encoding/EOL round-trips are exact |
| 2 | **Fast startup** — below perceptible delay on modern hardware |
| 3 | **Stability** — boringly crash-free |
| 4 | **Small binary** — single `.exe`, no installer required for portable use |
| 5 | **Predictable UX** — nothing surprises the user |
| 6 | **Clean codebase** — easy to read, modify, and audit |

## Non-Goals

Rivet explicitly **will not** have:

- Plugin system
- Docking layout / split views
- Embedded terminal
- LSP / language intelligence
- Macro recorder
- Auto-updater

## Feature Scope (MVP)

- Multi-tab editing with session restore
- Open / save with correct encoding and line-ending handling
  (UTF-8, UTF-8 BOM, UTF-16 LE/BE; LF / CRLF preservation)
- Find & Replace (with regex) + Go To Line
- Syntax highlighting for a curated language set
  (plain text, JSON, XML, INI, YAML, PowerShell, Python, JavaScript, HTML/CSS, C/C++,
  Rust, Bash, SQL, Makefile, Batch, Diff, Markdown)
- Large File Mode (>50 MB) — disables heavy features automatically
- Dark mode (View > Dark Mode); per-monitor DPI v2
- Session auto-checkpoint every 30 seconds (crash protection)
- Keyboard-only operation for all commands

## Build

**Requirements**

| Tool | Notes |
|------|-------|
| [rustup](https://rustup.rs/) | installs the Rust toolchain |
| MSVC build tools | Visual Studio 2019+ or Build Tools; required by the MSVC linker |
| Windows 10/11 x64 | only supported target |
| `SciLexer.dll` | Scintilla v5.x; place alongside `rivet.exe` at runtime |

```powershell
# 1. Install Rust (if not already present)
winget install Rustlang.Rustup
# or download the installer from https://rustup.rs/

# 2. Add the Windows MSVC target (rustup does this automatically on Windows)
rustup target add x86_64-pc-windows-msvc

# 3. Clone and build
git clone https://github.com/mgelsinger/rivet.git
cd rivet
cargo build --release
```

The release binary is written to:

```
target\x86_64-pc-windows-msvc\release\rivet.exe
```

At runtime, `rivet.exe` loads `SciLexer.dll` (Scintilla v5.x) from its own
directory.  Download the DLL from the
[Scintilla project](https://www.scintilla.org/) and place it alongside
`rivet.exe`.

### CI

GitHub Actions runs three gates on every push / PR to `main`:

1. `cargo fmt --check` — formatting must match `rustfmt.toml`
2. `cargo clippy -- -D warnings` — zero lint warnings tolerated
3. `cargo test` — all tests must pass

A release binary artefact is produced on every successful merge to `main`.
Pushing a `v*` tag additionally creates a GitHub Release with a packaged zip.

### Optional: cargo-deny

```powershell
cargo install cargo-deny
cargo deny check
```

Enforces the licence allowlist and vulnerability advisory policy in `deny.toml`.

## Project Structure

```
rivet/
├── src/
│   ├── main.rs               # entry point + module declarations
│   ├── app.rs                # App + DocumentState: document model, encoding, EOL
│   ├── languages.rs          # Language enum, language detection, keyword tables
│   ├── theme.rs              # Palette struct, LIGHT/DARK constants, apply_theme
│   ├── error.rs              # RivetError (Win32, Encoding, Io, ScintillaMsg)
│   ├── editor/
│   │   └── scintilla/
│   │       ├── mod.rs        # SciDll (DLL owner), ScintillaView (Scintilla HWND)
│   │       └── messages.rs   # SCI_* / SC_* message constants
│   ├── platform/
│   │   └── win32/
│   │       ├── mod.rs        # module re-exports
│   │       ├── dpi.rs        # Per-Monitor DPI v2 helpers
│   │       ├── dialogs.rs    # show_open_dialog, show_save_dialog
│   │       └── window.rs     # WindowState, wnd_proc, menus, session integration
│   └── session/
│       └── mod.rs            # SessionFile, save(), load() — APPDATA\Rivet\session.json
├── docs/
│   ├── ARCHITECTURE.md       # module boundaries and threading model
│   └── phase*.md             # per-phase task records
├── .cargo/
│   └── config.toml           # default target + CRT static link flag
├── .github/
│   └── workflows/
│       └── ci.yml            # CI: check + release-build + GitHub Release on tags
├── Cargo.toml
├── rust-toolchain.toml       # pinned stable toolchain
├── rustfmt.toml
└── deny.toml                 # cargo-deny policy
```

## Contributing

- CI must be green before any merge.
- Format with `cargo fmt` before committing.
- New `unsafe` code is only permitted inside `platform::win32` and
  `editor::scintilla`; every `unsafe` block must carry a `// SAFETY:` comment.
- Keep PRs focused: one logical change per PR.

## License

Licensed under either of

- [MIT License](LICENSE-MIT)
- [Apache License, Version 2.0](LICENSE-APACHE)

at your option.
