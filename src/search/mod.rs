// ── Search options ────────────────────────────────────────────────────────────
//
// Pure-Rust struct mirroring the FINDREPLACEW dialog flags.
// No Win32 imports; usable from any module.

/// Parameters for a single search operation.
///
/// Populated from the Win32 Find / Replace dialog flags and stored so that
/// F3 / Shift+F3 can repeat the last search without re-opening the dialog.
pub(crate) struct SearchOptions {
    pub(crate) text:       String,
    pub(crate) match_case: bool,
    pub(crate) whole_word: bool,
    pub(crate) forward:    bool,
}
