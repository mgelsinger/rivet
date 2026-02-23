// ── Win32 platform implementation ─────────────────────────────────────────────
//
// This is one of exactly two modules in the codebase where `unsafe` code is
// permitted (the other is `editor::scintilla`).  Every `unsafe` block MUST
// carry a `// SAFETY:` comment that states:
//   • which invariant makes the operation sound, and
//   • what the caller is responsible for maintaining.
//
// Nothing in this module is `pub` beyond what callers genuinely need; keep the
// unsafe surface as small as possible.

#![allow(unsafe_code)]
// Items below are stubs whose users arrive in Phase 2.
#![allow(dead_code)]

// ── Scintilla DLL constants ───────────────────────────────────────────────────

/// File name of the Scintilla DLL, expected beside the running executable.
///
/// Rivet uses the DLL-hosting approach rather than a compiled static lib:
/// `LoadLibraryW(SCINTILLA_DLL_NAME)` on startup registers the `"Scintilla"`
/// window class, after which child windows of that class are created with
/// `CreateWindowExW`.  See `editor::scintilla` for the hosting layer.
pub(crate) const SCINTILLA_DLL_NAME: &str = "SciLexer.dll";

/// Win32 window class registered by the Scintilla DLL on load.
pub(crate) const SCINTILLA_CLASS_NAME: &str = "Scintilla";

// ── Sub-modules (populated Phase 2+) ─────────────────────────────────────────

// pub mod window;   // Phase 2: main window, WndProc, message loop
// pub mod dialogs;  // Phase 3: common open/save/find dialogs
// pub mod dpi;      // Phase 8: per-monitor DPI v2 helpers
