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

// ── Document content ──────────────────────────────────────────────────────────

/// Replace all document text.  WPARAM = 0; LPARAM = pointer to null-terminated
/// UTF-8 string.  Resets the undo history and the save point.
pub(super) const SCI_SETTEXT: u32 = 2181;

/// Return the number of bytes in the document (not counting the terminating
/// null).  WPARAM and LPARAM are 0.
pub(super) const SCI_GETLENGTH: u32 = 2006;

/// Copy document bytes into a buffer.  WPARAM = buffer length (including null);
/// LPARAM = pointer to the buffer.  Returns the number of bytes written.
pub(super) const SCI_GETTEXT: u32 = 2182;

/// Mark the current state as the save point.  After this call
/// `SCN_SAVEPOINTLEFT` fires on the next edit and `SCN_SAVEPOINTREACHED`
/// fires when the user undoes back to this state.
pub(super) const SCI_SETSAVEPOINT: u32 = 2014;

// ── Lexer / Large File Mode ───────────────────────────────────────────────────

/// Set the lexer by numeric ID.  Pass `SCLEX_NULL` to disable highlighting.
pub(super) const SCI_SETLEXER: u32 = 4001;

/// Lexer ID for plain text (no highlighting).  Used in Large File Mode.
pub(super) const SCLEX_NULL: usize = 1;

// ── Word wrap ─────────────────────────────────────────────────────────────────

/// Set word-wrap mode.  Pass `SC_WRAP_NONE` or `SC_WRAP_WORD`.
pub(super) const SCI_SETWRAPMODE: u32 = 2268;

/// Disable word wrap.
pub(super) const SC_WRAP_NONE: usize = 0;

// ── Caret / position queries ──────────────────────────────────────────────────

/// Return the byte position of the caret.
pub(super) const SCI_GETCURRENTPOS: u32 = 2008;

/// Convert a byte position to a line number (0-based).
/// WPARAM = position.
pub(super) const SCI_LINEFROMPOSITION: u32 = 2166;

/// Return the visible column of a position on its line, taking tabs into
/// account.  WPARAM = position.
pub(super) const SCI_GETCOLUMN: u32 = 2129;

// ── EOL mode ─────────────────────────────────────────────────────────────────

/// Return the current EOL mode: `SC_EOL_CRLF`, `SC_EOL_LF`, or `SC_EOL_CR`.
pub(super) const SCI_GETEOLMODE: u32 = 2030;

/// Set the EOL mode used for new lines.  WPARAM = one of the SC_EOL_* values.
pub(super) const SCI_SETEOLMODE: u32 = 2031;

/// EOL mode: Windows `\r\n`.
pub(super) const SC_EOL_CRLF: isize = 0;
/// EOL mode: Unix `\n`.
pub(super) const SC_EOL_LF: isize = 1;
/// EOL mode: old Mac `\r`.
pub(super) const SC_EOL_CR: isize = 2;

// ── Notifications (via WM_NOTIFY / SCN_*) ─────────────────────────────────────
//
// These are pub(crate) because the WM_NOTIFY dispatch in `platform::win32::window`
// needs to match against them directly.

/// Notification code sent when the caret moves or the selection changes.
/// `SCNotification.updated` carries a bitmask of what changed.
pub(crate) const SCN_UPDATEUI: u32 = 2007;

/// Notification code sent when the document is first edited after a save point.
pub(crate) const SCN_SAVEPOINTLEFT: u32 = 2001;

/// Notification code sent when the document returns to a save point (e.g. undo).
pub(crate) const SCN_SAVEPOINTREACHED: u32 = 2002;
