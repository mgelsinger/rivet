// ── Scintilla child-window hosting ────────────────────────────────────────────
//
// This is one of exactly two modules in the codebase where `unsafe` code is
// permitted (the other is `platform::win32`).  Every `unsafe` block MUST
// carry a `// SAFETY:` comment.
//
// ── Integration decision (Phase 1) ───────────────────────────────────────────
//
// Approach chosen: **DLL hosting** (`SciLexer.dll`).
//
// Alternatives considered:
//
//   1. Static lib — compile Scintilla's C++ via `cc` crate in `build.rs`.
//      Pro: single self-contained `.exe`.
//      Con: requires `cl.exe` / `clang-cl` in the toolchain; significant
//           build-time increase; more complex CI.
//
//   2. DLL hosting — `LoadLibraryW("SciLexer.dll")` at startup; Scintilla
//      registers the `"Scintilla"` window class; child windows are created
//      with `CreateWindowExW`.
//      Pro: idiomatic Scintilla usage on Windows; simple build; Scintilla
//           team ships ready-to-use DLLs with each release.
//      Con: two files in the portable zip (`rivet.exe` + `SciLexer.dll`).
//
//   Decision: DLL hosting for MVP.  The safe abstraction boundary here is
//   identical regardless of the linking strategy, so switching later is a
//   build-system change only, not a code change.
//
// ── Scintilla version target ──────────────────────────────────────────────────
//
// Scintilla 5.x (latest stable at time of integration).
// Minimum required: 5.2 (introduces the `SCI_COUNTCHARACTERS` API we need
// for correct UTF-8 character counting independent of byte offsets).
//
// ── Sub-modules (populated Phase 2+) ─────────────────────────────────────────

#![allow(unsafe_code)]
// Stubs populated in Phase 2; allow list removed as each item gains a user.
#![allow(dead_code)]

// pub mod messages;  // Phase 2: type-safe SCI_* message wrappers
//
// pub struct ScintillaView { … }  // Phase 2: HWND-based view type
