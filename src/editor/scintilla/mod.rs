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
    SC_CP_UTF8, SC_EOL_CR, SC_EOL_CRLF, SC_EOL_LF, SC_WRAP_NONE, SCLEX_NULL, SCI_GETCOLUMN,
    SCI_GETCURRENTPOS, SCI_GETEOLMODE, SCI_GETFIRSTVISIBLELINE, SCI_GETLENGTH, SCI_GETTEXT,
    SCI_GOTOPOS, SCI_LINEFROMPOSITION, SCI_SETCODEPAGE, SCI_SETFIRSTVISIBLELINE, SCI_SETLEXER,
    SCI_SETSAVEPOINT, SCI_SETTEXT, SCI_SETWRAPMODE, SCI_SETEOLMODE,
};

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{GetLastError, HINSTANCE, HMODULE, HWND, LPARAM, WPARAM},
        System::LibraryLoader::{FreeLibrary, LoadLibraryW},
        UI::WindowsAndMessaging::{
            CreateWindowExW, DestroyWindow, SendMessageW, ShowWindow, HMENU, SW_HIDE, SW_SHOW,
            WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
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
}
