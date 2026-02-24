// ── Scintilla message constants ───────────────────────────────────────────────
//
// Source of truth: Scintilla.h (https://www.scintilla.org/ScintillaDoc.html)
// Only the subset needed for the current phase is listed here; add more as
// features are implemented rather than importing the full header.
//
// All SCI_* values are sent via SendMessageW(hwnd_sci, SCI_*, wparam, lparam).

// ── Code page ─────────────────────────────────────────────────────────────────

/// Set the code page used to interpret the document bytes as characters.
/// Pass `SC_CP_UTF8` as WPARAM to enable UTF-8 mode.
pub(super) const SCI_SETCODEPAGE: u32 = 2037;

/// UTF-8 code page; use as WPARAM with `SCI_SETCODEPAGE`.
pub(super) const SC_CP_UTF8: usize = 65001;
