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

// ── Sub-modules ───────────────────────────────────────────────────────────────

pub mod dialogs; // Phase 3: common open/save/find dialogs
pub mod window; // Phase 2: main window, WndProc, message loop

pub(crate) mod dpi; // Phase 8: per-monitor DPI v2 helpers
