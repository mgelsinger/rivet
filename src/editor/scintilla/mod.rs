// ── Scintilla child-window hosting ────────────────────────────────────────────
//
// This is one of exactly two modules in the codebase where `unsafe` code is
// permitted (the other is `platform::win32`).  Every `unsafe` block MUST
// carry a `// SAFETY:` comment.
//
// ── Integration decision (Phase 1 — confirmed Phase 2b) ──────────────────────
//
// Approach: DLL hosting (`SciLexer.dll`).
//
//   LoadLibraryW("SciLexer.dll")  →  registers "Scintilla" window class
//   CreateWindowExW("Scintilla")  →  creates the editor child window
//   SendMessageW(hwnd, SCI_*, …)  →  all editor operations
//
// `ScintillaView` owns both the child HWND and the DLL `HMODULE`.
// `FreeLibrary` is called from `Drop`; by that point, Windows has already
// destroyed the child HWND as part of parent-window destruction.
//
// ── Security note ─────────────────────────────────────────────────────────────
//
// Phase 2b uses `LoadLibraryW("SciLexer.dll")` (filename only), which resolves
// to the application directory first on Win10/11.
// Phase 10 will harden this with `LoadLibraryExW` + full path to prevent
// DLL-hijacking on machines with attacker-writable directories in the search path.

#![allow(unsafe_code)]

pub mod messages;

use messages::{
    SC_CP_UTF8, SC_EOL_CR, SC_EOL_CRLF, SC_EOL_LF, SC_WRAP_NONE, SCLEX_NULL, SCI_GETCOLUMN,
    SCI_GETCURRENTPOS, SCI_GETEOLMODE, SCI_GETLENGTH, SCI_GETTEXT, SCI_LINEFROMPOSITION,
    SCI_SETCODEPAGE, SCI_SETLEXER, SCI_SETSAVEPOINT, SCI_SETTEXT, SCI_SETWRAPMODE,
};

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{GetLastError, HINSTANCE, HMODULE, HWND, LPARAM, WPARAM},
        System::LibraryLoader::{FreeLibrary, LoadLibraryW},
        UI::WindowsAndMessaging::{
            CreateWindowExW, SendMessageW, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE, WS_CHILD,
            WS_CLIPSIBLINGS, WS_VISIBLE,
        },
    },
};

use crate::{
    app::EolMode,
    error::{Result, RivetError},
};

// ── DLL / class identity ──────────────────────────────────────────────────────

/// File name of the Scintilla DLL, expected beside the running executable.
const DLL_NAME: &str = "SciLexer.dll";

/// Win32 window class registered by the Scintilla DLL on load.
const CLASS_NAME: &str = "Scintilla";

// ── ScintillaView ─────────────────────────────────────────────────────────────

/// A hosted Scintilla editor child window.
///
/// Owns the `SciLexer.dll` module handle; `FreeLibrary` is called on `Drop`.
/// The child `HWND` itself is destroyed automatically by Windows when the
/// parent window is destroyed — `Drop` does **not** call `DestroyWindow`.
pub(crate) struct ScintillaView {
    hwnd: HWND,
    dll: HMODULE,
}

impl ScintillaView {
    /// Load `SciLexer.dll` and create a Scintilla child window inside
    /// `hwnd_parent`.
    ///
    /// The window is created with zero size; `WM_SIZE` in the parent's
    /// WndProc is responsible for calling `SetWindowPos` to give it its
    /// real dimensions.
    pub(crate) fn create(hwnd_parent: HWND, hinstance: HINSTANCE) -> Result<Self> {
        // Build a null-terminated UTF-16 DLL path (filename only; Windows
        // searches the application directory first).
        let dll_path: Vec<u16> = DLL_NAME
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: dll_path is a valid null-terminated UTF-16 string.
        // LoadLibraryW registers the "Scintilla" window class on success.
        let dll =
            unsafe { LoadLibraryW(PCWSTR(dll_path.as_ptr())) }.map_err(RivetError::from)?;

        let class_wide: Vec<u16> =
            CLASS_NAME.encode_utf16().chain(std::iter::once(0)).collect();

        // SAFETY: class_wide is a valid null-terminated UTF-16 string of the
        // class name just registered by SciLexer.dll.
        // hwnd_parent is the valid main-window HWND provided by WM_CREATE.
        // hinstance is the exe's HINSTANCE obtained from GetModuleHandleW.
        // Initial position/size (0,0,0,0) — WM_SIZE will give real dimensions.
        let hwnd = unsafe {
            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_wide.as_ptr()),
                PCWSTR::null(),
                WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WINDOW_STYLE(0x0200_0000), // WS_CLIPCHILDREN
                0,
                0,
                0,
                0,
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
            // Unload the DLL since we could not create the child window.
            // SAFETY: dll was successfully loaded above and not freed yet.
            unsafe { let _ = FreeLibrary(dll); }
            return Err(RivetError::Win32 {
                function: "CreateWindowExW (Scintilla)",
                code,
            });
        }

        // ── Basic initialisation ──────────────────────────────────────────────

        // SAFETY: hwnd is a valid Scintilla window returned above.
        // SCI_SETCODEPAGE with SC_CP_UTF8 is a documented, safe initialisation.
        unsafe {
            let _ = SendMessageW(hwnd, SCI_SETCODEPAGE, WPARAM(SC_CP_UTF8), LPARAM(0));
        }

        Ok(Self { hwnd, dll })
    }

    /// Returns the Scintilla child window handle.
    ///
    /// The handle is valid until the parent window is destroyed.
    pub(crate) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    // ── Document operations ───────────────────────────────────────────────────

    /// Replace all document text with the given UTF-8 bytes and reset the
    /// undo history and save point.
    ///
    /// `text` must be valid UTF-8; the caller is responsible for transcoding
    /// non-UTF-8 source files before calling this method.
    pub(crate) fn set_text(&self, text: &[u8]) {
        // Build a null-terminated copy — SCI_SETTEXT expects a C string.
        let mut buf: Vec<u8> = Vec::with_capacity(text.len() + 1);
        buf.extend_from_slice(text);
        buf.push(0);

        // SAFETY: hwnd is a valid Scintilla HWND.  buf is a null-terminated
        // UTF-8 byte sequence that outlives this call.  SCI_SETTEXT with
        // WPARAM(0) and a valid LPARAM string pointer is documented behaviour.
        unsafe {
            let _ = SendMessageW(
                self.hwnd,
                SCI_SETTEXT,
                WPARAM(0),
                LPARAM(buf.as_ptr() as isize),
            );
        }
    }

    /// Mark the current document state as the save point (clears the dirty flag
    /// in Scintilla's internal model and arms the `SCN_SAVEPOINTLEFT` notification).
    pub(crate) fn set_save_point(&self) {
        // SAFETY: hwnd is a valid Scintilla HWND.  SCI_SETSAVEPOINT takes no
        // parameters; WPARAM and LPARAM are ignored.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETSAVEPOINT, WPARAM(0), LPARAM(0));
        }
    }

    /// Enter or exit Large File Mode.
    ///
    /// In large-file mode the lexer is set to `SCLEX_NULL` (plain text) and
    /// word wrap is disabled, both of which are required to keep Scintilla
    /// responsive on huge files.
    pub(crate) fn set_large_file_mode(&self, enable: bool) {
        if enable {
            // SAFETY: hwnd valid; SCI_SETLEXER with SCLEX_NULL is documented.
            unsafe {
                let _ = SendMessageW(self.hwnd, SCI_SETLEXER, WPARAM(SCLEX_NULL), LPARAM(0));
                let _ =
                    SendMessageW(self.hwnd, SCI_SETWRAPMODE, WPARAM(SC_WRAP_NONE), LPARAM(0));
            }
        }
        // Restoring the lexer / wrap mode after disabling large-file mode is
        // deferred to Phase 7 (syntax highlighting).
    }

    /// Read the full document text as UTF-8 bytes (without null terminator).
    pub(crate) fn get_text(&self) -> Vec<u8> {
        // SAFETY: hwnd is a valid Scintilla HWND.
        // SCI_GETLENGTH returns the byte count (excluding null terminator).
        let len =
            unsafe { SendMessageW(self.hwnd, SCI_GETLENGTH, WPARAM(0), LPARAM(0)).0 as usize };

        // Allocate len + 1 to hold the null terminator Scintilla writes.
        let mut buf = vec![0u8; len + 1];

        // SAFETY: buf is len+1 bytes and outlives this call.
        // SCI_GETTEXT with WPARAM = buffer size (including null) and
        // LPARAM = pointer to buffer is the documented read pattern.
        unsafe {
            let _ = SendMessageW(
                self.hwnd,
                SCI_GETTEXT,
                WPARAM(len + 1),
                LPARAM(buf.as_mut_ptr() as isize),
            );
        }

        buf.truncate(len); // drop the trailing null
        buf
    }

    // ── Caret / position queries ──────────────────────────────────────────────

    /// Return the current caret position as a `(line, column)` pair
    /// (both **1-based** for status-bar display).
    ///
    /// `column` counts character columns, taking tab stops into account.
    pub(crate) fn caret_line_col(&self) -> (usize, usize) {
        // SAFETY: hwnd is a valid Scintilla HWND.  All three messages are
        // documented read-only queries with no side effects.
        unsafe {
            let pos = SendMessageW(self.hwnd, SCI_GETCURRENTPOS, WPARAM(0), LPARAM(0)).0 as usize;
            let line =
                SendMessageW(self.hwnd, SCI_LINEFROMPOSITION, WPARAM(pos), LPARAM(0)).0 as usize;
            let col =
                SendMessageW(self.hwnd, SCI_GETCOLUMN, WPARAM(pos), LPARAM(0)).0 as usize;
            (line + 1, col + 1) // convert to 1-based
        }
    }

    /// Return the EOL mode currently set in the document.
    pub(crate) fn eol_mode(&self) -> EolMode {
        // SAFETY: hwnd is a valid Scintilla HWND.  SCI_GETEOLMODE is a
        // documented read-only query.
        let mode = unsafe {
            SendMessageW(self.hwnd, SCI_GETEOLMODE, WPARAM(0), LPARAM(0)).0
        };
        match mode {
            x if x == SC_EOL_CRLF => EolMode::Crlf,
            x if x == SC_EOL_LF => EolMode::Lf,
            x if x == SC_EOL_CR => EolMode::Cr,
            _ => EolMode::Crlf, // defensive fallback
        }
    }

    /// Set the EOL mode used for newly inserted line endings.
    pub(crate) fn set_eol_mode(&self, eol: EolMode) {
        let mode = match eol {
            EolMode::Crlf => SC_EOL_CRLF,
            EolMode::Lf => SC_EOL_LF,
            EolMode::Cr => SC_EOL_CR,
        };
        // SAFETY: hwnd valid; SCI_SETEOLMODE with a valid SC_EOL_* value is
        // documented behaviour.
        unsafe {
            let _ = SendMessageW(self.hwnd, SCI_SETEOLMODE, WPARAM(mode as usize), LPARAM(0));
        }
    }
}

impl Drop for ScintillaView {
    fn drop(&mut self) {
        // SAFETY: self.dll was successfully returned by LoadLibraryW (verified
        // in create()) and has not been freed since.  When Drop runs the
        // Scintilla HWND has already been destroyed by Windows (child windows
        // are destroyed before WM_DESTROY fires on the parent), so the DLL's
        // window-procedure is no longer reachable.
        unsafe {
            let _ = FreeLibrary(self.dll);
        }
    }
}
