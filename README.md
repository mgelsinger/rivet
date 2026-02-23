# Rivet

A simple, fast, and correct text editor for Windows 10/11 x64.

> **Status:** early development (Phase 0 — scaffolding)

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
- Find & Replace (with regex) + Find in Files with cancellation
- Syntax highlighting for a curated language set
  (plain text, JSON, XML, INI, YAML, PowerShell, Python, JS/TS, HTML/CSS, C/C++)
- Large File Mode (>50 MB) — disables heavy features automatically
- Dark mode; per-monitor DPI v2
- Keyboard-only operation for all commands

## Build

**Requirements**

| Tool | Notes |
|------|-------|
| [rustup](https://rustup.rs/) | installs the Rust toolchain |
| MSVC build tools | Visual Studio 2019+ or Build Tools; required by the MSVC linker |
| Windows 10/11 x64 | only supported target |

```powershell
# 1. Install Rust (if not already present)
winget install Rustlang.Rustup
# or download the installer from https://rustup.rs/

# 2. Add the Windows MSVC target (rustup does this automatically on Windows)
rustup target add x86_64-pc-windows-msvc

# 3. Clone and build
git clone https://github.com/YOUR_ORG/rivet.git
cd rivet
cargo build --release
```

The release binary is written to:

```
target\x86_64-pc-windows-msvc\release\rivet.exe
```

### CI

GitHub Actions runs three gates on every push / PR to `main`:

1. `cargo fmt --check` — formatting must match `rustfmt.toml`
2. `cargo clippy -- -D warnings` — zero lint warnings tolerated
3. `cargo test` — all tests must pass

A release binary artefact is produced on every successful merge to `main`.

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
│   └── main.rs           # entry point
├── docs/
│   └── ARCHITECTURE.md   # module boundaries and threading model
├── .cargo/
│   └── config.toml       # default target + CRT static link flag
├── .github/
│   └── workflows/
│       └── ci.yml        # CI pipeline
├── Cargo.toml
├── rust-toolchain.toml   # pinned stable toolchain
├── rustfmt.toml
└── deny.toml             # cargo-deny policy
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
