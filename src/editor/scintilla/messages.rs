// ── Scintilla message constants ───────────────────────────────────────────────
//
// Source of truth: Scintilla.h (https://www.scintilla.org/ScintillaDoc.html)
// Only the subset needed for the current phase is listed here.
// All SCI_* values are sent via SendMessageW(hwnd_sci, SCI_*, wparam, lparam).

// ── Code page ─────────────────────────────────────────────────────────────────

/// Set the code page.  Pass `SC_CP_UTF8` as WPARAM.
pub(super) const SCI_SETCODEPAGE: u32 = 2037;
/// UTF-8 code page value for `SCI_SETCODEPAGE`.
pub(super) const SC_CP_UTF8: usize = 65001;

// ── Document content ──────────────────────────────────────────────────────────

/// Replace all document text.  WPARAM=0; LPARAM=null-terminated UTF-8 string.
pub(super) const SCI_SETTEXT: u32 = 2181;
/// Return byte count of the document (excluding null terminator).
pub(super) const SCI_GETLENGTH: u32 = 2006;
/// Copy document bytes.  WPARAM=buffer len (incl. null); LPARAM=buffer ptr.
pub(super) const SCI_GETTEXT: u32 = 2182;
/// Mark the current state as the save point.
pub(super) const SCI_SETSAVEPOINT: u32 = 2014;

// ── Lexer / Large File Mode ───────────────────────────────────────────────────

/// Set lexer by numeric ID.
pub(super) const SCI_SETLEXER: u32 = 4001;
/// Plain-text lexer (no highlighting).
pub(super) const SCLEX_NULL: usize = 1;

// ── Word wrap ─────────────────────────────────────────────────────────────────

/// Set word-wrap mode.
pub(super) const SCI_SETWRAPMODE: u32 = 2268;
/// Get word-wrap mode.
pub(super) const SCI_GETWRAPMODE: u32 = 2269;
/// Disable word wrap.
pub(super) const SC_WRAP_NONE: usize = 0;
/// Wrap at word boundaries.
pub(super) const SC_WRAP_WORD: usize = 1;

// ── Caret / position ──────────────────────────────────────────────────────────

/// Return the byte position of the caret.
pub(super) const SCI_GETCURRENTPOS: u32 = 2008;
/// Move the caret to a byte position (also scrolls into view).
pub(super) const SCI_GOTOPOS: u32 = 2025;
/// Convert a byte position to a 0-based line number.
pub(super) const SCI_LINEFROMPOSITION: u32 = 2166;
/// Return the visible column of a position (tab-aware).
pub(super) const SCI_GETCOLUMN: u32 = 2129;

// ── Scroll ────────────────────────────────────────────────────────────────────

/// Return the index of the first visible line (0-based).
pub(super) const SCI_GETFIRSTVISIBLELINE: u32 = 2152;
/// Set the first visible line.  WPARAM = line index.
pub(super) const SCI_SETFIRSTVISIBLELINE: u32 = 2613;

// ── EOL mode ─────────────────────────────────────────────────────────────────

/// Return the current EOL mode.
pub(super) const SCI_GETEOLMODE: u32 = 2030;
/// Set the EOL mode.  WPARAM = SC_EOL_*.
pub(super) const SCI_SETEOLMODE: u32 = 2031;

/// EOL mode: Windows `\r\n`.
pub(super) const SC_EOL_CRLF: isize = 0;
/// EOL mode: Unix `\n`.
pub(super) const SC_EOL_LF: isize = 1;
/// EOL mode: old Mac `\r`.
pub(super) const SC_EOL_CR: isize = 2;

// ── Edit operations ───────────────────────────────────────────────────────────

/// Undo the last action (Scintilla-specific; Scintilla also accepts WM_UNDO).
pub(super) const SCI_UNDO: u32 = 2176;
/// Redo the last undone action (no standard Win32 equivalent).
pub(super) const SCI_REDO: u32 = 2179;
/// Select all document text.
pub(super) const SCI_SELECTALL: u32 = 2013;
/// Convert existing EOL sequences to the mode given in WPARAM (SC_EOL_*).
pub(super) const SCI_CONVERTEOLS: u32 = 2029;

// Standard Win32 clipboard messages — Scintilla processes these natively.
/// Cut selection to clipboard.
pub(super) const WM_CUT:   u32 = 0x0300;
/// Copy selection to clipboard.
pub(super) const WM_COPY:  u32 = 0x0301;
/// Paste from clipboard.
pub(super) const WM_PASTE: u32 = 0x0302;
/// Delete selection without copying.
pub(super) const WM_CLEAR: u32 = 0x0303;
/// Undo last action (Win32 standard; Scintilla also processes this).
pub(super) const WM_UNDO:  u32 = 0x0304;

// ── Find / target ─────────────────────────────────────────────────────────────

/// Set the search flags used by `SCI_SEARCHINTARGET`.  WPARAM = flag bitmask.
pub(super) const SCI_SETSEARCHFLAGS: u32 = 2188;
/// Set target start position.  WPARAM = byte position.
pub(super) const SCI_SETTARGETSTART: u32 = 2190;
/// Get target start position.
pub(super) const SCI_GETTARGETSTART: u32 = 2191;
/// Set target end position.  WPARAM = byte position.
pub(super) const SCI_SETTARGETEND:   u32 = 2192;
/// Get target end position.
pub(super) const SCI_GETTARGETEND:   u32 = 2193;
/// Search for text in the target range.  WPARAM = text length; LPARAM = text ptr.
/// Returns match start position, or -1 if not found.
/// If targetStart > targetEnd the search is backward.
pub(super) const SCI_SEARCHINTARGET: u32 = 2185;
/// Replace the target text.  WPARAM = replacement length; LPARAM = text ptr.
/// Returns the length of the replacement.
pub(super) const SCI_REPLACETARGET:  u32 = 2194;

// ── Selection ─────────────────────────────────────────────────────────────────

/// Return the byte position of the selection anchor.
pub(super) const SCI_GETSELECTIONSTART: u32 = 2143;
/// Return the byte position of the selection caret end.
pub(super) const SCI_GETSELECTIONEND:   u32 = 2145;
/// Set both the anchor and caret, then scroll into view.
/// WPARAM = anchor position; LPARAM = caret position.
pub(super) const SCI_SETSEL:            u32 = 2163;
/// Scroll to make the caret visible.
pub(super) const SCI_SCROLLCARET:       u32 = 2169;

// ── Undo grouping ─────────────────────────────────────────────────────────────

/// Start a compound (grouped) undo action.
pub(super) const SCI_BEGINUNDOACTION:   u32 = 2078;
/// End a compound undo action.
pub(super) const SCI_ENDUNDOACTION:     u32 = 2079;

// ── Go To Line ───────────────────────────────────────────────────────────────

/// Return the total number of lines in the document.
pub(super) const SCI_GETLINECOUNT:      u32 = 2154;
/// Return the byte position of the start of `line` (0-based).  WPARAM = line.
pub(super) const SCI_POSITIONFROMLINE:  u32 = 2167;

// ── Find flags (pub(crate) for use in window.rs) ──────────────────────────────

/// Case-sensitive search flag for `SCI_SETSEARCHFLAGS`.
pub(crate) const SCFIND_MATCHCASE: u32 = 0x0000_0004;
/// Whole-word-only search flag for `SCI_SETSEARCHFLAGS`.
pub(crate) const SCFIND_WHOLEWORD: u32 = 0x0000_0002;

// ── Notifications — pub(crate) for WM_NOTIFY dispatch in window.rs ────────────

/// Caret moved or selection changed.
pub(crate) const SCN_UPDATEUI: u32 = 2007;
/// Document first edited after a save point.
pub(crate) const SCN_SAVEPOINTLEFT: u32 = 2001;
/// Document returned to a save point (e.g. undo).
pub(crate) const SCN_SAVEPOINTREACHED: u32 = 2002;
