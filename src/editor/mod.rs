// ── Editor component abstraction ──────────────────────────────────────────────
//
// Exposes a safe Rust API over the underlying Scintilla editor control.
// Callers interact with `ScintillaView` (defined in `scintilla::`) through
// the public methods on this module; they never touch Win32 handles directly.

// Items below are stubs whose users arrive in Phase 2+.
#![allow(dead_code)]

pub mod scintilla;

// ── Large-file threshold ──────────────────────────────────────────────────────

/// Files larger than this byte count are opened in **Large File Mode**:
///
/// * Word-wrap is disabled.
/// * Full syntax highlighting is replaced by plain-text lexing.
/// * Session checkpoints save metadata only (no file content).
/// * A status-bar indicator is shown to inform the user.
///
/// Adjust this constant to tune the trade-off between features and
/// performance on the target machine class.
pub(crate) const LARGE_FILE_THRESHOLD_BYTES: u64 = 50 * 1_024 * 1_024; // 50 MiB
