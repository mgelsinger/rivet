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

use messages::{SC_CP_UTF8, SCI_SETCODEPAGE};

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{GetLastError, HINSTANCE, HMODULE, HWND, LPARAM, WPARAM},
        System::LibraryLoader::{FreeLibrary, LoadLibraryW},
        UI::WindowsAndMessaging::{
            CreateWindowExW, SendMessageW, HMENU, WINDOW_EX_STYLE, WINDOW_STYLE,
            WS_CHILD, WS_CLIPSIBLINGS, WS_VISIBLE,
        },
    },
};

use crate::error::{Result, RivetError};

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
