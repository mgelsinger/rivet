// ── Scintilla child-window hosting ────────────────────────────────────────────
//
// This is one of exactly two modules where `unsafe` is permitted.
// Every `unsafe` block MUST carry a `// SAFETY:` comment.
//
// ── DLL ownership model (Phase 4) ─────────────────────────────────────────────
//
// `SciDll` owns the single `LoadLibraryW` call for `SciLexer.dll`.  It is
// stored in `WindowState` and lives longer than all `ScintillaView` instances.
// `ScintillaView` holds only a child `HWND`; it no longer owns the DLL.
//
// Drop order inside `WindowState` (Rust drops fields in declaration order):
//   1. `app` (pure Rust, no HWNDs) — dropped first
//   2. `sci_views` — structs with stale HWNDs (Windows already destroyed them
//      as part of parent-window teardown before WM_DESTROY fired); no-op drop
//   3. `sci_dll` — `FreeLibrary` called here, after all windows are gone ✓
//
// ── Security note ─────────────────────────────────────────────────────────────
//
// `SciDll::load()` calls `LoadLibraryW("SciLexer.dll")` (filename only).
// Windows resolves this to the application directory first on Win10/11.
// Phase 10 will harden this to `LoadLibraryExW` with a full path.

#![allow(unsafe_code)]

pub mod messages;

use messages::{
    SC_CP_UTF8, SC_EOL_CR, SC_EOL_CRLF, SC_EOL_LF, SC_WRAP_NONE, SC_WRAP_WORD, SCLEX_NULL,
    SCI_BEGINUNDOACTION, SCI_CONVERTEOLS, SCI_ENDUNDOACTION,
    SCI_GETCOLUMN, SCI_GETCURRENTPOS, SCI_GETEOLMODE, SCI_GETFIRSTVISIBLELINE,
    SCI_GETLENGTH, SCI_GETLINECOUNT, SCI_GETSELECTIONEND, SCI_GETSELECTIONSTART,
    SCI_GETTARGETEND, SCI_GETTEXT, SCI_GETWRAPMODE,
    SCI_GOTOPOS, SCI_LINEFROMPOSITION, SCI_POSITIONFROMLINE,
    SCI_REDO, SCI_REPLACETARGET, SCI_SCROLLCARET,
    SCI_SEARCHINTARGET, SCI_SELECTALL, SCI_SETCODEPAGE, SCI_SETFIRSTVISIBLELINE, SCI_SETLEXER,
    SCI_SETSAVEPOINT, SCI_SETSEARCHFLAGS, SCI_SETSEL,
    SCI_SETTARGETEND, SCI_SETTARGETSTART, SCI_SETTEXT, SCI_SETWRAPMODE, SCI_SETEOLMODE,
    WM_CLEAR, WM_COPY, WM_CUT, WM_PASTE, WM_UNDO,
};

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{GetLastError, HINSTANCE, HMODULE, HWND, LPARAM, WPARAM},
        System::LibraryLoader::{FreeLibrary, LoadLibraryW},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, ShowWindow, HMENU, SW_HIDE, SW_SHOW,
            WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_CLIPSIBLINGS,
        },
    },
};

use crate::{
    app::EolMode,
    error::{Result, RivetError},
};

// ── DLL identity ──────────────────────────────────────────────────────────────

const DLL_NAME: &str = "SciLexer.dll";
const CLASS_NAME: &str = "Scintilla";

// ── SciDll ────────────────────────────────────────────────────────────────────

/// RAII handle to the loaded `SciLexer.dll`.
///
/// Loading the DLL causes it to register the `"Scintilla"` window class.
/// `FreeLibrary` is called on `Drop`, which should happen after all
/// `ScintillaView` child windows have been destroyed.
pub(crate) struct SciDll(HMODULE);

impl SciDll {
    /// Load `SciLexer.dll` from the application directory.
    ///
    /// This also registers the `"Scintilla"` Win32 window class, making it
    /// available for `ScintillaView::create`.
    pub(crate) fn load() -> Result<Self> {
        let path: Vec<u16> = DLL_NAME.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: path is a valid null-terminated UTF-16 string.
        // LoadLibraryW searches the application directory first on Win10/11.
        let dll = unsafe { LoadLibraryW(PCWSTR(path.as_ptr())) }.map_err(RivetError::from)?;
        Ok(Self(dll))
    }
}

impl Drop for SciDll {
    fn drop(&mut self) {
        // SAFETY: self.0 was returned by a successful LoadLibraryW and has not
        // been freed since.  All ScintillaView HWNDs are already destroyed
        // (Windows destroys child windows before WM_DESTROY fires on the parent,
        // and WindowState field order ensures sci_views drops before sci_dll).
        unsafe {
            let _ = FreeLibrary(self.0);
        }
    }
}

// ── ScintillaView ─────────────────────────────────────────────────────────────

/// A hosted Scintilla editor child window.
///
/// Does **not** own the `SciLexer.dll` module handle — that is owned by
/// `SciDll` in `WindowState`.  The child `HWND` is destroyed automatically
/// by Windows when the parent is destroyed; no explicit cleanup is needed.
pub(crate) struct ScintillaView {
    hwnd: HWND,
}

impl ScintillaView {
    /// Create a Scintilla child window inside `hwnd_parent`.
    ///
    /// `_dll` proves that `SciLexer.dll` is loaded and the `"Scintilla"` class
    /// is registered.  The window is created hidden with zero size; call
    /// `show(true)` and `SetWindowPos` to make it visible.
    pub(crate) fn create(
        hwnd_parent: HWND,
        hinstance: HINSTANCE,
        _dll: &SciDll,
    ) -> Result<Self> {
        let class_wide: Vec<u16> =
            CLASS_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        // SAFETY: class_wide is null-terminated UTF-16 for the class registered
        // by SciLexer.dll (_dll proves the DLL is loaded).  hwnd_parent and
        // hinstance are valid Win32 handles from WM_CREATE.
        // New views start hidden (no WS_VISIBLE) so only the active tab is shown.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_wide.as_ptr()),
                PCWSTR::null(),
                WS_CHILD | WS_CLIPSIBLINGS | WINDOW_STYLE(0x0200_0000), // WS_CLIPCHILDREN
                0, 0, 0, 0,
                hwnd_parent,
                HMENU::default(),
                hinstance,
                None,
            )
        };

        if hwnd == HWND::default() {
            // SAFETY: GetLastError reads thread-local state set by the just-
            // failed CreateWindowExW; no Win32 calls between them.
            let code = unsafe { GetLastError().0 };
            return Err(RivetError::Win32 { function: "CreateWindowExW (Scintilla)", code });
        }

        // SAFETY: hwnd is a valid Scintilla window.  SCI_SETCODEPAGE with
        // SC_CP_UTF8 is documented safe initialisation.
        unsafe {
            let _ = SendMessageW(hwnd, SCI_SETCODEPAGE, WPARAM(SC_CP_UTF8), LPARAM(0));
        }

        Ok(Self { hwnd })
    }

    /// The Scintilla child window handle.  Valid until the parent is destroyed.
    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Show or hide this Scintilla view.  Used when switching tabs.
    pub(crate) fn show(&self, visible: bool) {
        let cmd = if visible { SW_SHOW } else { SW_HIDE };
        // SAFETY: hwnd is a valid child window handle.
        // ShowWindow return value (previous visibility) is intentionally unused.
        unsafe {
            let _ = ShowWindow(self.hwnd, cmd);
        }
    }

    /// Explicitly destroy the child HWND.
    ///
    /// Call this when closing a tab while the parent window is still alive.
    /// After this call the `ScintillaView` must not be used; it should be
    /// dropped immediately.
    pub(crate) fn destroy(&self) {
        // SAFETY: hwnd is a valid child window whose parent is still alive.
        // DestroyWindow on a child triggers WM_DESTROY for the child only —
        // no application WM_QUIT is posted.
        unsafe {
            let _ = DestroyWindow(self.hwnd);
        }
    }

    // ── Document operations ───────────────────────────────────────────────────

    /// Replace all document text (UTF-8) and reset the undo history and save point.
    pub(crate) fn set_text(&self, text: &[u8]) {
        let mut buf: Vec<u8> = Vec::with_capacity(text.len() + 1);
        buf.extend_from_slice(text);
        buf.push(0);
        // SAFETY: hwnd valid; buf is null-terminated UTF-8 that outlives the call.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETTEXT, WPARAM(0), LPARAM(buf.as_ptr() as isize));
        }
    }

    /// Read the full document text as UTF-8 bytes (without null terminator).
    pub(crate) fn get_text(&self) -> Vec<u8> {
        // SAFETY: hwnd valid; SCI_GETLENGTH is a read-only query.
        let len = unsafe {
            SendMessageW(self.hwnd, SCI_GETLENGTH, WPARAM(0), LPARAM(0)).0 as usize
        };
        let mut buf = vec![0u8; len + 1];
        // SAFETY: buf is len+1 bytes; SCI_GETTEXT with matching buffer size is safe.
        unsafe {
            let _ = SendMessageW(
                self.hwnd, SCI_GETTEXT,
                WPARAM(len + 1), LPARAM(buf.as_mut_ptr() as isize),
            );
        }
        buf.truncate(len);
        buf
    }

    /// Mark the current state as the save point.
    pub(crate) fn set_save_point(&self) {
        // SAFETY: hwnd valid; SCI_SETSAVEPOINT takes no parameters.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETSAVEPOINT, WPARAM(0), LPARAM(0));
        }
    }

    /// Enable or disable Large File Mode (plain-text lexer, no word wrap).
    pub(crate) fn set_large_file_mode(&self, enable: bool) {
        if enable {
            // SAFETY: hwnd valid; documented Scintilla messages.
            unsafe {
                let _ = SendMessageW(self.hwnd, SCI_SETLEXER, WPARAM(SCLEX_NULL), LPARAM(0));
                let _ = SendMessageW(self.hwnd, SCI_SETWRAPMODE, WPARAM(SC_WRAP_NONE), LPARAM(0));
            }
        }
    }

    // ── Caret / position ──────────────────────────────────────────────────────

    /// Raw byte offset of the caret (for session persistence).
    pub(crate) fn caret_pos(&self) -> usize {
        // SAFETY: hwnd valid; read-only query.
        unsafe { SendMessageW(self.hwnd, SCI_GETCURRENTPOS, WPARAM(0), LPARAM(0)).0 as usize }
    }

    /// Move the caret to a byte offset.
    pub(crate) fn set_caret_pos(&self, pos: usize) {
        // SAFETY: hwnd valid; SCI_GOTOPOS with a valid position is safe.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_GOTOPOS, WPARAM(pos), LPARAM(0));
        }
    }

    /// First visible line index (0-based, for session persistence).
    pub(crate) fn first_visible_line(&self) -> usize {
        // SAFETY: hwnd valid; read-only query.
        unsafe {
            SendMessageW(self.hwnd, SCI_GETFIRSTVISIBLELINE, WPARAM(0), LPARAM(0)).0 as usize
        }
    }

    /// Scroll to make `line` (0-based) the first visible line.
    pub(crate) fn set_first_visible_line(&self, line: usize) {
        // SAFETY: hwnd valid; documented Scintilla scroll message.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETFIRSTVISIBLELINE, WPARAM(line), LPARAM(0));
        }
    }

    /// 1-based (line, column) for status-bar display.
    pub(crate) fn caret_line_col(&self) -> (usize, usize) {
        // SAFETY: hwnd valid; all three are read-only queries.
        unsafe {
            let pos = SendMessageW(self.hwnd, SCI_GETCURRENTPOS, WPARAM(0), LPARAM(0)).0 as usize;
            let line = SendMessageW(self.hwnd, SCI_LINEFROMPOSITION, WPARAM(pos), LPARAM(0)).0 as usize;
            let col  = SendMessageW(self.hwnd, SCI_GETCOLUMN, WPARAM(pos), LPARAM(0)).0 as usize;
            (line + 1, col + 1)
        }
    }

    /// Current EOL mode.
    pub(crate) fn eol_mode(&self) -> EolMode {
        // SAFETY: hwnd valid; read-only query.
        let mode = unsafe { SendMessageW(self.hwnd, SCI_GETEOLMODE, WPARAM(0), LPARAM(0)).0 };
        match mode {
            x if x == SC_EOL_CRLF => EolMode::Crlf,
            x if x == SC_EOL_LF   => EolMode::Lf,
            x if x == SC_EOL_CR   => EolMode::Cr,
            _                     => EolMode::Crlf,
        }
    }

    /// Set the EOL mode for new lines.
    pub(crate) fn set_eol_mode(&self, eol: EolMode) {
        let mode = match eol {
            EolMode::Crlf => SC_EOL_CRLF,
            EolMode::Lf   => SC_EOL_LF,
            EolMode::Cr   => SC_EOL_CR,
        };
        // SAFETY: hwnd valid; SCI_SETEOLMODE with a valid SC_EOL_* is documented.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETEOLMODE, WPARAM(mode as usize), LPARAM(0));
        }
    }

    // ── Edit operations ───────────────────────────────────────────────────────

    /// Undo the last action.
    pub(crate) fn undo(&self) {
        // SAFETY: hwnd valid; WM_UNDO is a standard Win32 message Scintilla handles.
        unsafe { let _ = SendMessageW(self.hwnd, WM_UNDO, WPARAM(0), LPARAM(0)); }
    }

    /// Redo the last undone action.
    pub(crate) fn redo(&self) {
        // SAFETY: hwnd valid; SCI_REDO takes no parameters.
        unsafe { let _ = SendMessageW(self.hwnd, SCI_REDO, WPARAM(0), LPARAM(0)); }
    }

    /// Cut the current selection to the clipboard.
    pub(crate) fn cut(&self) {
        // SAFETY: hwnd valid; WM_CUT is processed natively by Scintilla.
        unsafe { let _ = SendMessageW(self.hwnd, WM_CUT, WPARAM(0), LPARAM(0)); }
    }

    /// Copy the current selection to the clipboard.
    pub(crate) fn copy_to_clipboard(&self) {
        // SAFETY: hwnd valid; WM_COPY is processed natively by Scintilla.
        unsafe { let _ = SendMessageW(self.hwnd, WM_COPY, WPARAM(0), LPARAM(0)); }
    }

    /// Paste from the clipboard at the caret position.
    pub(crate) fn paste(&self) {
        // SAFETY: hwnd valid; WM_PASTE is processed natively by Scintilla.
        unsafe { let _ = SendMessageW(self.hwnd, WM_PASTE, WPARAM(0), LPARAM(0)); }
    }

    /// Delete the current selection without copying to the clipboard.
    pub(crate) fn delete_selection(&self) {
        // SAFETY: hwnd valid; WM_CLEAR is processed natively by Scintilla.
        unsafe { let _ = SendMessageW(self.hwnd, WM_CLEAR, WPARAM(0), LPARAM(0)); }
    }

    /// Select all document text.
    pub(crate) fn select_all(&self) {
        // SAFETY: hwnd valid; SCI_SELECTALL takes no parameters.
        unsafe { let _ = SendMessageW(self.hwnd, SCI_SELECTALL, WPARAM(0), LPARAM(0)); }
    }

    /// Convert all existing EOL sequences in the document to `eol`.
    ///
    /// This modifies the document content (triggers `SCN_SAVEPOINTLEFT`).
    /// Call `set_eol_mode` afterwards so that new keystrokes also use the new style.
    pub(crate) fn convert_eols(&self, eol: EolMode) {
        let mode = match eol {
            EolMode::Crlf => SC_EOL_CRLF,
            EolMode::Lf   => SC_EOL_LF,
            EolMode::Cr   => SC_EOL_CR,
        };
        // SAFETY: hwnd valid; SCI_CONVERTEOLS with a valid SC_EOL_* value is documented.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_CONVERTEOLS, WPARAM(mode as usize), LPARAM(0));
        }
    }

    /// Enable or disable word wrapping for this view.
    pub(crate) fn set_word_wrap(&self, enabled: bool) {
        let mode = if enabled { SC_WRAP_WORD } else { SC_WRAP_NONE };
        // SAFETY: hwnd valid; SCI_SETWRAPMODE with SC_WRAP_WORD / SC_WRAP_NONE is documented.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETWRAPMODE, WPARAM(mode), LPARAM(0));
        }
    }

    /// Return `true` if word wrap is currently enabled.
    pub(crate) fn is_word_wrap(&self) -> bool {
        // SAFETY: hwnd valid; SCI_GETWRAPMODE is a read-only query.
        let mode = unsafe {
            SendMessageW(self.hwnd, SCI_GETWRAPMODE, WPARAM(0), LPARAM(0)).0 as usize
        };
        mode != SC_WRAP_NONE
    }

    // ── Document length ───────────────────────────────────────────────────────

    /// Total byte length of the document (excluding null terminator).
    pub(crate) fn doc_len(&self) -> usize {
        // SAFETY: hwnd valid; read-only query.
        unsafe { SendMessageW(self.hwnd, SCI_GETLENGTH, WPARAM(0), LPARAM(0)).0 as usize }
    }

    // ── Find / replace ────────────────────────────────────────────────────────

    /// Set the target range for `search_in_target`.
    ///
    /// Pass `start > end` for a backward search.
    pub(crate) fn set_target(&self, start: usize, end: usize) {
        // SAFETY: hwnd valid; documented Scintilla target messages.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETTARGETSTART, WPARAM(start), LPARAM(0));
            let _ = SendMessageW(self.hwnd, SCI_SETTARGETEND,   WPARAM(end),   LPARAM(0));
        }
    }

    /// Search for `text` (UTF-8) in the current target range.
    ///
    /// Returns the byte position of the match start, or `None` if not found.
    /// On success the target range is updated to the match extent.
    pub(crate) fn search_in_target(&self, text: &[u8], flags: u32) -> Option<usize> {
        // SAFETY: hwnd valid; text is valid UTF-8 that outlives the SendMessageW call.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETSEARCHFLAGS, WPARAM(flags as usize), LPARAM(0));
            let result = SendMessageW(
                self.hwnd,
                SCI_SEARCHINTARGET,
                WPARAM(text.len()),
                LPARAM(text.as_ptr() as isize),
            ).0;
            if result < 0 { None } else { Some(result as usize) }
        }
    }

    /// Return the byte position of the end of the last target (after a successful search).
    pub(crate) fn get_target_end(&self) -> usize {
        // SAFETY: hwnd valid; read-only query.
        unsafe { SendMessageW(self.hwnd, SCI_GETTARGETEND, WPARAM(0), LPARAM(0)).0 as usize }
    }

    /// Replace the current target range with `text` (UTF-8).
    ///
    /// Returns the byte length of the replacement text.
    pub(crate) fn replace_target(&self, text: &[u8]) -> usize {
        // SAFETY: hwnd valid; text is valid UTF-8 that outlives the call.
        unsafe {
            SendMessageW(
                self.hwnd,
                SCI_REPLACETARGET,
                WPARAM(text.len()),
                LPARAM(text.as_ptr() as isize),
            ).0 as usize
        }
    }

    // ── Selection ─────────────────────────────────────────────────────────────

    /// Byte position of the selection anchor (the non-moving end).
    pub(crate) fn selection_start(&self) -> usize {
        // SAFETY: hwnd valid; read-only query.
        unsafe { SendMessageW(self.hwnd, SCI_GETSELECTIONSTART, WPARAM(0), LPARAM(0)).0 as usize }
    }

    /// Byte position of the selection caret (the moving end).
    pub(crate) fn selection_end(&self) -> usize {
        // SAFETY: hwnd valid; read-only query.
        unsafe { SendMessageW(self.hwnd, SCI_GETSELECTIONEND, WPARAM(0), LPARAM(0)).0 as usize }
    }

    /// Set the selection anchor and caret, then scroll the caret into view.
    pub(crate) fn set_sel(&self, anchor: usize, caret: usize) {
        // SAFETY: hwnd valid; SCI_SETSEL with valid positions is documented safe.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETSEL, WPARAM(anchor), LPARAM(caret as isize));
        }
    }

    /// Scroll to make the caret visible.
    pub(crate) fn scroll_caret(&self) {
        // SAFETY: hwnd valid; SCI_SCROLLCARET takes no parameters.
        unsafe { let _ = SendMessageW(self.hwnd, SCI_SCROLLCARET, WPARAM(0), LPARAM(0)); }
    }

    // ── Undo grouping ─────────────────────────────────────────────────────────

    /// Begin a compound undo action (multiple edits become one Ctrl+Z step).
    pub(crate) fn begin_undo_action(&self) {
        // SAFETY: hwnd valid; SCI_BEGINUNDOACTION takes no parameters.
        unsafe { let _ = SendMessageW(self.hwnd, SCI_BEGINUNDOACTION, WPARAM(0), LPARAM(0)); }
    }

    /// End the compound undo action started by `begin_undo_action`.
    pub(crate) fn end_undo_action(&self) {
        // SAFETY: hwnd valid; SCI_ENDUNDOACTION takes no parameters.
        unsafe { let _ = SendMessageW(self.hwnd, SCI_ENDUNDOACTION, WPARAM(0), LPARAM(0)); }
    }

    // ── Go To Line ────────────────────────────────────────────────────────────

    /// Total number of lines in the document (always ≥ 1).
    pub(crate) fn line_count(&self) -> usize {
        // SAFETY: hwnd valid; read-only query.
        let n = unsafe { SendMessageW(self.hwnd, SCI_GETLINECOUNT, WPARAM(0), LPARAM(0)).0 };
        (n as usize).max(1)
    }

    /// Byte position of the first character on `line` (0-based).
    pub(crate) fn position_from_line(&self, line: usize) -> usize {
        // SAFETY: hwnd valid; Scintilla clamps out-of-range lines to the last line.
        unsafe {
            SendMessageW(self.hwnd, SCI_POSITIONFROMLINE, WPARAM(line), LPARAM(0)).0 as usize
        }
    }

    // ── High-level search ─────────────────────────────────────────────────────

    /// Find `text` (UTF-8) from the current selection, wrapping around.
    ///
    /// Returns `true` if a match was found and selected.
    /// For backward search pass `forward = false`.
    pub(crate) fn find_next(&self, text: &[u8], flags: u32, forward: bool) -> bool {
        let doc_len   = self.doc_len();
        let sel_start = self.selection_start();
        let sel_end   = self.selection_end();

        if forward {
            // Primary: from end of selection to end of document.
            self.set_target(sel_end, doc_len);
            if let Some(pos) = self.search_in_target(text, flags) {
                let end = self.get_target_end();
                self.set_sel(pos, end);
                self.scroll_caret();
                return true;
            }
            // Wrap: from start of document to start of selection.
            if sel_start > 0 {
                self.set_target(0, sel_start);
                if let Some(pos) = self.search_in_target(text, flags) {
                    let end = self.get_target_end();
                    self.set_sel(pos, end);
                    self.scroll_caret();
                    return true;
                }
            }
        } else {
            // Backward: reversed target (targetStart > targetEnd) tells Scintilla to search backward.
            // Primary: from just before the current selection back to the start.
            if sel_start > 0 {
                self.set_target(sel_start, 0);
                if let Some(pos) = self.search_in_target(text, flags) {
                    let end = self.get_target_end();
                    self.set_sel(pos, end);
                    self.scroll_caret();
                    return true;
                }
            }
            // Wrap: from end of document back to end of current selection.
            if sel_end < doc_len {
                self.set_target(doc_len, sel_end);
                if let Some(pos) = self.search_in_target(text, flags) {
                    let end = self.get_target_end();
                    self.set_sel(pos, end);
                    self.scroll_caret();
                    return true;
                }
            }
        }
        false
    }

    /// Replace every occurrence of `find` with `replacement` in one undo action.
    ///
    /// Returns the number of replacements made.
    pub(crate) fn replace_all(&self, find: &[u8], replacement: &[u8], flags: u32) -> usize {
        let mut count = 0usize;
        let mut pos   = 0usize;
        self.begin_undo_action();
        loop {
            let doc_len = self.doc_len(); // recalculate: doc size changes after each replacement
            self.set_target(pos, doc_len);
            match self.search_in_target(find, flags) {
                None => break,
                Some(match_start) => {
                    let repl_len = self.replace_target(replacement);
                    pos   = match_start + repl_len;
                    count += 1;
                }
            }
        }
        self.end_undo_action();
        count
    }
}
