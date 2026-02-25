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

/// Set lexer via ILexer5* (Scintilla 5.x / Lexilla).
/// WPARAM = 0; LPARAM = ILexer5* from Lexilla CreateLexer(), or 0 for plain text.
pub(super) const SCI_SETILEXER: u32 = 4033;

// ── Style operation messages ───────────────────────────────────────────────────

pub(super) const SCI_STYLECLEARALL: u32 = 2050;
pub(super) const SCI_STYLESETFORE: u32 = 2051;
pub(super) const SCI_STYLESETBACK: u32 = 2052;
pub(super) const SCI_STYLESETBOLD: u32 = 2053;
#[allow(dead_code)]
pub(super) const SCI_STYLESETITALIC: u32 = 2054;
pub(super) const SCI_STYLESETSIZE: u32 = 2055;
pub(super) const SCI_STYLESETFONT: u32 = 2056;
pub(super) const SCI_SETKEYWORDS: u32 = 4005;

// ── Special style slot IDs ────────────────────────────────────────────────────

pub(crate) const STYLE_DEFAULT: u32 = 32;
pub(crate) const STYLE_LINENUMBER: u32 = 33;
#[allow(dead_code)]
pub(crate) const STYLE_BRACELIGHT: u32 = 34;

// ── SCE_* style numbers — SCLEX_CPP ──────────────────────────────────────────

pub(crate) const SCE_C_COMMENT: u32 = 1;
pub(crate) const SCE_C_COMMENTLINE: u32 = 2;
pub(crate) const SCE_C_COMMENTDOC: u32 = 3;
pub(crate) const SCE_C_NUMBER: u32 = 4;
pub(crate) const SCE_C_WORD: u32 = 5;
pub(crate) const SCE_C_STRING: u32 = 6;
pub(crate) const SCE_C_CHARACTER: u32 = 7;
pub(crate) const SCE_C_PREPROCESSOR: u32 = 9;
pub(crate) const SCE_C_OPERATOR: u32 = 10;
#[allow(dead_code)]
pub(crate) const SCE_C_IDENTIFIER: u32 = 11;
#[allow(dead_code)]
pub(crate) const SCE_C_STRINGEOL: u32 = 12;
#[allow(dead_code)]
pub(crate) const SCE_C_VERBATIM: u32 = 13;
pub(crate) const SCE_C_REGEX: u32 = 14;
pub(crate) const SCE_C_WORD2: u32 = 16;

// ── SCE_* style numbers — SCLEX_PYTHON ───────────────────────────────────────

pub(crate) const SCE_P_COMMENTLINE: u32 = 1;
pub(crate) const SCE_P_NUMBER: u32 = 2;
pub(crate) const SCE_P_STRING: u32 = 3;
pub(crate) const SCE_P_CHARACTER: u32 = 4;
pub(crate) const SCE_P_WORD: u32 = 5;
pub(crate) const SCE_P_TRIPLE: u32 = 6;
pub(crate) const SCE_P_TRIPLEDOUBLE: u32 = 7;
pub(crate) const SCE_P_CLASSNAME: u32 = 8;
pub(crate) const SCE_P_DEFNAME: u32 = 9;
pub(crate) const SCE_P_OPERATOR: u32 = 10;
#[allow(dead_code)]
pub(crate) const SCE_P_IDENTIFIER: u32 = 11;
pub(crate) const SCE_P_DECORATOR: u32 = 15;

// ── SCE_* style numbers — SCLEX_RUST ─────────────────────────────────────────

pub(crate) const SCE_RUST_COMMENTBLOCK: u32 = 1;
pub(crate) const SCE_RUST_COMMENTLINE: u32 = 2;
pub(crate) const SCE_RUST_COMMENTBLOCKDOC: u32 = 3;
pub(crate) const SCE_RUST_COMMENTLINEDOC: u32 = 4;
pub(crate) const SCE_RUST_NUMBER: u32 = 5;
pub(crate) const SCE_RUST_WORD: u32 = 6;
pub(crate) const SCE_RUST_WORD2: u32 = 7;
pub(crate) const SCE_RUST_STRING: u32 = 13;
pub(crate) const SCE_RUST_STRINGR: u32 = 14;
pub(crate) const SCE_RUST_CHARACTER: u32 = 15;
pub(crate) const SCE_RUST_OPERATOR: u32 = 16;
#[allow(dead_code)]
pub(crate) const SCE_RUST_IDENTIFIER: u32 = 17;
pub(crate) const SCE_RUST_LIFETIME: u32 = 18;
pub(crate) const SCE_RUST_MACRO: u32 = 19;
#[allow(dead_code)]
pub(crate) const SCE_RUST_LEXERROR: u32 = 20;

// ── SCE_* style numbers — SCLEX_HTML / SCLEX_XML (share SCE_H_*) ─────────────

pub(crate) const SCE_H_TAG: u32 = 1;
#[allow(dead_code)]
pub(crate) const SCE_H_TAGUNKNOWN: u32 = 2;
pub(crate) const SCE_H_ATTRIBUTE: u32 = 3;
#[allow(dead_code)]
pub(crate) const SCE_H_ATTRIBUTEUNKNOWN: u32 = 4;
#[allow(dead_code)]
pub(crate) const SCE_H_NUMBER: u32 = 5;
pub(crate) const SCE_H_DOUBLESTRING: u32 = 6;
pub(crate) const SCE_H_SINGLESTRING: u32 = 7;
#[allow(dead_code)]
pub(crate) const SCE_H_OTHER: u32 = 8;
pub(crate) const SCE_H_COMMENT: u32 = 9;
#[allow(dead_code)]
pub(crate) const SCE_H_ENTITY: u32 = 10;
pub(crate) const SCE_H_TAGEND: u32 = 11;

// ── SCE_* style numbers — SCLEX_CSS ──────────────────────────────────────────

pub(crate) const SCE_CSS_TAG: u32 = 1;
pub(crate) const SCE_CSS_CLASS: u32 = 2;
pub(crate) const SCE_CSS_PSEUDOCLASS: u32 = 3;
#[allow(dead_code)]
pub(crate) const SCE_CSS_UNKNOWN_PSEUDOCLASS: u32 = 4;
pub(crate) const SCE_CSS_OPERATOR: u32 = 5;
pub(crate) const SCE_CSS_IDENTIFIER: u32 = 6;
#[allow(dead_code)]
pub(crate) const SCE_CSS_UNKNOWN_IDENTIFIER: u32 = 7;
pub(crate) const SCE_CSS_VALUE: u32 = 8;
pub(crate) const SCE_CSS_COMMENT: u32 = 9;
pub(crate) const SCE_CSS_ID: u32 = 10;
pub(crate) const SCE_CSS_IMPORTANT: u32 = 11;
pub(crate) const SCE_CSS_SINGLESTRING: u32 = 13;
pub(crate) const SCE_CSS_DOUBLESTRING: u32 = 14;
#[allow(dead_code)]
pub(crate) const SCE_CSS_ATTRIBUTE: u32 = 15;

// ── SCE_* style numbers — SCLEX_JSON ─────────────────────────────────────────

pub(crate) const SCE_JSON_NUMBER: u32 = 1;
pub(crate) const SCE_JSON_STRING: u32 = 2;
#[allow(dead_code)]
pub(crate) const SCE_JSON_STRINGEOL: u32 = 3;
pub(crate) const SCE_JSON_PROPERTYNAME: u32 = 4;
#[allow(dead_code)]
pub(crate) const SCE_JSON_ESCAPESEQUENCE: u32 = 5;
#[allow(dead_code)]
pub(crate) const SCE_JSON_LINECOMMENT: u32 = 6;
#[allow(dead_code)]
pub(crate) const SCE_JSON_BLOCKCOMMENT: u32 = 7;
pub(crate) const SCE_JSON_OPERATOR: u32 = 8;
pub(crate) const SCE_JSON_KEYWORD: u32 = 11;
#[allow(dead_code)]
pub(crate) const SCE_JSON_LDKEYWORD: u32 = 12;
#[allow(dead_code)]
pub(crate) const SCE_JSON_ERROR: u32 = 13;

// ── SCE_* style numbers — SCLEX_SQL ──────────────────────────────────────────

pub(crate) const SCE_SQL_COMMENT: u32 = 1;
pub(crate) const SCE_SQL_COMMENTLINE: u32 = 2;
pub(crate) const SCE_SQL_COMMENTDOC: u32 = 3;
pub(crate) const SCE_SQL_NUMBER: u32 = 4;
pub(crate) const SCE_SQL_WORD: u32 = 5;
pub(crate) const SCE_SQL_STRING: u32 = 6;
pub(crate) const SCE_SQL_CHARACTER: u32 = 7;
#[allow(dead_code)]
pub(crate) const SCE_SQL_SQLPLUS: u32 = 8;
pub(crate) const SCE_SQL_OPERATOR: u32 = 10;
#[allow(dead_code)]
pub(crate) const SCE_SQL_IDENTIFIER: u32 = 11;
#[allow(dead_code)]
pub(crate) const SCE_SQL_QUOTEDIDENTIFIER: u32 = 23;

// ── SCE_* style numbers — SCLEX_TOML ─────────────────────────────────────────

pub(crate) const SCE_TOML_COMMENT: u32 = 1;
pub(crate) const SCE_TOML_SECTIONTITLE: u32 = 2;
pub(crate) const SCE_TOML_KEY: u32 = 4;
#[allow(dead_code)]
pub(crate) const SCE_TOML_ASSIGNMENT: u32 = 5;
pub(crate) const SCE_TOML_NUMBER: u32 = 6;
pub(crate) const SCE_TOML_STRING: u32 = 7;
pub(crate) const SCE_TOML_STRINGMULTILINE: u32 = 8;
pub(crate) const SCE_TOML_BOOL: u32 = 9;

// ── SCE_* style numbers — SCLEX_PROPERTIES ───────────────────────────────────

pub(crate) const SCE_PROPS_COMMENT: u32 = 1;
pub(crate) const SCE_PROPS_SECTION: u32 = 2;
#[allow(dead_code)]
pub(crate) const SCE_PROPS_ASSIGNMENT: u32 = 3;
#[allow(dead_code)]
pub(crate) const SCE_PROPS_DEFVAL: u32 = 4;
pub(crate) const SCE_PROPS_KEY: u32 = 5;

// ── SCE_* style numbers — SCLEX_BATCH ────────────────────────────────────────

pub(crate) const SCE_BAT_COMMENT: u32 = 1;
pub(crate) const SCE_BAT_WORD: u32 = 2;
pub(crate) const SCE_BAT_LABEL: u32 = 3;
#[allow(dead_code)]
pub(crate) const SCE_BAT_HIDE: u32 = 4;
pub(crate) const SCE_BAT_COMMAND: u32 = 5;
#[allow(dead_code)]
pub(crate) const SCE_BAT_IDENTIFIER: u32 = 6;
pub(crate) const SCE_BAT_OPERATOR: u32 = 7;

// ── SCE_* style numbers — SCLEX_MAKEFILE ─────────────────────────────────────

pub(crate) const SCE_MAKE_COMMENT: u32 = 1;
pub(crate) const SCE_MAKE_PREPROCESSOR: u32 = 2;
#[allow(dead_code)]
pub(crate) const SCE_MAKE_IDEOL: u32 = 9;
pub(crate) const SCE_MAKE_TARGET: u32 = 11;
pub(crate) const SCE_MAKE_OPERATOR: u32 = 12;

// ── SCE_* style numbers — SCLEX_DIFF ─────────────────────────────────────────

pub(crate) const SCE_DIFF_COMMENT: u32 = 1;
pub(crate) const SCE_DIFF_COMMAND: u32 = 2;
pub(crate) const SCE_DIFF_HEADER: u32 = 3;
pub(crate) const SCE_DIFF_POSITION: u32 = 4;
pub(crate) const SCE_DIFF_DELETED: u32 = 5;
pub(crate) const SCE_DIFF_ADDED: u32 = 6;
#[allow(dead_code)]
pub(crate) const SCE_DIFF_CHANGED: u32 = 7;

// ── SCE_* style numbers — SCLEX_BASH ─────────────────────────────────────────

pub(crate) const SCE_SH_COMMENTLINE: u32 = 2;
pub(crate) const SCE_SH_NUMBER: u32 = 3;
pub(crate) const SCE_SH_WORD: u32 = 4;
pub(crate) const SCE_SH_STRING: u32 = 5;
pub(crate) const SCE_SH_CHARACTER: u32 = 6;
pub(crate) const SCE_SH_OPERATOR: u32 = 7;
#[allow(dead_code)]
pub(crate) const SCE_SH_IDENTIFIER: u32 = 8;
pub(crate) const SCE_SH_SCALAR: u32 = 9;
#[allow(dead_code)]
pub(crate) const SCE_SH_PARAM: u32 = 10;
#[allow(dead_code)]
pub(crate) const SCE_SH_BACKTICKS: u32 = 11;

// ── SCE_* style numbers — SCLEX_MARKDOWN ─────────────────────────────────────

pub(crate) const SCE_MARKDOWN_STRONG1: u32 = 2;
pub(crate) const SCE_MARKDOWN_STRONG2: u32 = 3;
pub(crate) const SCE_MARKDOWN_EM1: u32 = 4;
pub(crate) const SCE_MARKDOWN_EM2: u32 = 5;
pub(crate) const SCE_MARKDOWN_HEADER1: u32 = 6;
pub(crate) const SCE_MARKDOWN_HEADER2: u32 = 7;
pub(crate) const SCE_MARKDOWN_HEADER3: u32 = 8;
pub(crate) const SCE_MARKDOWN_HEADER4: u32 = 9;
pub(crate) const SCE_MARKDOWN_HEADER5: u32 = 10;
pub(crate) const SCE_MARKDOWN_HEADER6: u32 = 11;
#[allow(dead_code)]
pub(crate) const SCE_MARKDOWN_PRECHAR: u32 = 12;
pub(crate) const SCE_MARKDOWN_ULIST_ITEM: u32 = 13;
pub(crate) const SCE_MARKDOWN_OLIST_ITEM: u32 = 14;
pub(crate) const SCE_MARKDOWN_BLOCKQUOTE: u32 = 15;
pub(crate) const SCE_MARKDOWN_STRIKEOUT: u32 = 16;
pub(crate) const SCE_MARKDOWN_HRULE: u32 = 17;
pub(crate) const SCE_MARKDOWN_LINK: u32 = 18;
pub(crate) const SCE_MARKDOWN_CODE: u32 = 19;
pub(crate) const SCE_MARKDOWN_CODE2: u32 = 20;
pub(crate) const SCE_MARKDOWN_CODEBK: u32 = 21;

// ── SCE_* style numbers — SCLEX_YAML ─────────────────────────────────────────

pub(crate) const SCE_YAML_COMMENT: u32 = 1;
pub(crate) const SCE_YAML_IDENTIFIER: u32 = 2;
pub(crate) const SCE_YAML_KEYWORD: u32 = 3;
pub(crate) const SCE_YAML_NUMBER: u32 = 4;
#[allow(dead_code)]
pub(crate) const SCE_YAML_REFERENCE: u32 = 5;
pub(crate) const SCE_YAML_DOCUMENT: u32 = 6;
pub(crate) const SCE_YAML_TEXT: u32 = 7;
#[allow(dead_code)]
pub(crate) const SCE_YAML_ERROR: u32 = 8;
pub(crate) const SCE_YAML_OPERATOR: u32 = 9;

// ── SCE_* style numbers — SCLEX_POWERSHELL ───────────────────────────────────

pub(crate) const SCE_POWERSHELL_COMMENT: u32 = 1;
pub(crate) const SCE_POWERSHELL_STRING: u32 = 2;
pub(crate) const SCE_POWERSHELL_CHARACTER: u32 = 3;
pub(crate) const SCE_POWERSHELL_NUMBER: u32 = 4;
pub(crate) const SCE_POWERSHELL_VARIABLE: u32 = 5;
pub(crate) const SCE_POWERSHELL_OPERATOR: u32 = 6;
#[allow(dead_code)]
pub(crate) const SCE_POWERSHELL_IDENTIFIER: u32 = 7;
pub(crate) const SCE_POWERSHELL_KEYWORD: u32 = 8;
pub(crate) const SCE_POWERSHELL_CMDLET: u32 = 9;
#[allow(dead_code)]
pub(crate) const SCE_POWERSHELL_ALIAS: u32 = 10;
pub(crate) const SCE_POWERSHELL_FUNCTION: u32 = 11;
#[allow(dead_code)]
pub(crate) const SCE_POWERSHELL_USER1: u32 = 12;
pub(crate) const SCE_POWERSHELL_COMMENTSTREAM: u32 = 14;
pub(crate) const SCE_POWERSHELL_HERE_STRING: u32 = 15;
pub(crate) const SCE_POWERSHELL_HERE_CHARACTER: u32 = 16;
#[allow(dead_code)]
pub(crate) const SCE_POWERSHELL_COMMENTDOCKEYWORD: u32 = 17;

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
pub(super) const WM_CUT: u32 = 0x0300;
/// Copy selection to clipboard.
pub(super) const WM_COPY: u32 = 0x0301;
/// Paste from clipboard.
pub(super) const WM_PASTE: u32 = 0x0302;
/// Delete selection without copying.
pub(super) const WM_CLEAR: u32 = 0x0303;
/// Undo last action (Win32 standard; Scintilla also processes this).
pub(super) const WM_UNDO: u32 = 0x0304;

// ── Find / target ─────────────────────────────────────────────────────────────

/// Set the search flags used by `SCI_SEARCHINTARGET`.  WPARAM = flag bitmask.
pub(super) const SCI_SETSEARCHFLAGS: u32 = 2188;
/// Set target start position.  WPARAM = byte position.
pub(super) const SCI_SETTARGETSTART: u32 = 2190;
/// Get target start position.
pub(super) const SCI_GETTARGETSTART: u32 = 2191;
/// Set target end position.  WPARAM = byte position.
pub(super) const SCI_SETTARGETEND: u32 = 2192;
/// Get target end position.
pub(super) const SCI_GETTARGETEND: u32 = 2193;
/// Search for text in the target range.  WPARAM = text length; LPARAM = text ptr.
/// Returns match start position, or -1 if not found.
/// If targetStart > targetEnd the search is backward.
pub(super) const SCI_SEARCHINTARGET: u32 = 2185;
/// Replace the target text.  WPARAM = replacement length; LPARAM = text ptr.
/// Returns the length of the replacement.
pub(super) const SCI_REPLACETARGET: u32 = 2194;

// ── Selection ─────────────────────────────────────────────────────────────────

/// Return the byte position of the selection anchor.
pub(super) const SCI_GETSELECTIONSTART: u32 = 2143;
/// Return the byte position of the selection caret end.
pub(super) const SCI_GETSELECTIONEND: u32 = 2145;
/// Set both the anchor and caret, then scroll into view.
/// WPARAM = anchor position; LPARAM = caret position.
pub(super) const SCI_SETSEL: u32 = 2163;
/// Scroll to make the caret visible.
pub(super) const SCI_SCROLLCARET: u32 = 2169;

// ── Undo grouping ─────────────────────────────────────────────────────────────

/// Start a compound (grouped) undo action.
pub(super) const SCI_BEGINUNDOACTION: u32 = 2078;
/// End a compound undo action.
pub(super) const SCI_ENDUNDOACTION: u32 = 2079;

// ── Go To Line ───────────────────────────────────────────────────────────────

/// Return the total number of lines in the document.
pub(super) const SCI_GETLINECOUNT: u32 = 2154;
/// Return the byte position of the start of `line` (0-based).  WPARAM = line.
pub(super) const SCI_POSITIONFROMLINE: u32 = 2167;

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
