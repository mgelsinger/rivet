// ── Common dialogs ─────────────────────────────────────────────────────────────
//
// Thin wrappers around the Win32 common-dialog APIs.  Each function returns
// `Some(path)` on user confirmation and `None` on cancel or error.
//
// This is inside `platform::win32` so `unsafe` is permitted per crate policy.

#![allow(unsafe_code)]

use std::path::PathBuf;

use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::HWND,
        UI::Controls::Dialogs::{GetOpenFileNameW, GetSaveFileNameW, OPENFILENAMEW, OFN_FILEMUSTEXIST, OFN_HIDEREADONLY, OFN_OVERWRITEPROMPT, OFN_PATHMUSTEXIST},
    },
};

// ── Buffer size ───────────────────────────────────────────────────────────────

/// Maximum path length in `WCHAR`s, including the null terminator.
/// `MAX_PATH` (260) is too short for modern Windows paths; use 32 768 which
/// is the documented maximum for `\\?\` extended paths.
const PATH_BUF_LEN: usize = 32_768;

// ── Open dialog ───────────────────────────────────────────────────────────────

/// Show the standard "Open File" dialog.
///
/// Returns the chosen path, or `None` if the user cancelled.
pub(crate) fn show_open_dialog(hwnd_owner: HWND) -> Option<PathBuf> {
    let mut buf = vec![0u16; PATH_BUF_LEN];

    // The filter string is null-separated pairs ending with a double null:
    // "Display\0*.ext\0Display2\0*.ext2\0\0"
    let filter: Vec<u16> = "All Files (*.*)\0*.*\0Text Files (*.txt)\0*.txt\0\0"
        .encode_utf16()
        .collect();

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd_owner,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: windows::core::PWSTR(buf.as_mut_ptr()),
        nMaxFile: PATH_BUF_LEN as u32,
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY,
        ..Default::default()
    };

    // SAFETY: `ofn` is fully initialised; `buf` and `filter` outlive this
    // call.  GetOpenFileNameW reads and writes only within the buffers we
    // provided.  The function is called on the UI thread (required for modal
    // dialogs).
    let ok = unsafe { GetOpenFileNameW(&mut ofn) };

    if ok.as_bool() {
        Some(path_from_buf(&buf))
    } else {
        None
    }
}

// ── Save dialog ───────────────────────────────────────────────────────────────

/// Show the standard "Save As" dialog.
///
/// `default_name` pre-populates the filename field (pass an empty string or
/// the current filename).  Returns the chosen path, or `None` if cancelled.
pub(crate) fn show_save_dialog(hwnd_owner: HWND, default_name: &str) -> Option<PathBuf> {
    let mut buf: Vec<u16> = default_name
        .encode_utf16()
        .chain(std::iter::repeat(0).take(PATH_BUF_LEN))
        .take(PATH_BUF_LEN)
        .collect();

    let filter: Vec<u16> = "All Files (*.*)\0*.*\0Text Files (*.txt)\0*.txt\0\0"
        .encode_utf16()
        .collect();

    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: hwnd_owner,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: windows::core::PWSTR(buf.as_mut_ptr()),
        nMaxFile: PATH_BUF_LEN as u32,
        Flags: OFN_OVERWRITEPROMPT | OFN_PATHMUSTEXIST,
        ..Default::default()
    };

    // SAFETY: same invariants as show_open_dialog above.
    let ok = unsafe { GetSaveFileNameW(&mut ofn) };

    if ok.as_bool() {
        Some(path_from_buf(&buf))
    } else {
        None
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a null-terminated UTF-16 buffer to a `PathBuf`.
fn path_from_buf(buf: &[u16]) -> PathBuf {
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    PathBuf::from(String::from_utf16_lossy(&buf[..len]).as_ref())
}
