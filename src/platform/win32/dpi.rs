#![allow(unsafe_code)]

use windows::Win32::{
    Foundation::HWND,
    UI::HiDpi::{
        GetDpiForSystem, GetDpiForWindow, SetProcessDpiAwarenessContext,
        DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    },
};

pub(crate) const BASE_DPI: u32 = 96;

/// Scale a pixel value defined at 96 DPI to `dpi`.
pub(crate) fn scale(px: i32, dpi: u32) -> i32 {
    px * dpi as i32 / BASE_DPI as i32
}

/// Opt into Per-Monitor v2 DPI awareness.
/// MUST be called before any window is created on the calling thread.
pub(crate) fn init() {
    // SAFETY: Must precede all window creation; single call at process start.
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

/// Return the DPI for `hwnd`. Falls back to BASE_DPI (96) on failure.
pub(crate) fn get_for_window(hwnd: HWND) -> u32 {
    // SAFETY: hwnd is a valid window handle provided by the caller.
    let v = unsafe { GetDpiForWindow(hwnd) };
    if v == 0 {
        BASE_DPI
    } else {
        v
    }
}

/// Return the primary-monitor system DPI. Used before window creation.
pub(crate) fn get_system_dpi() -> u32 {
    // SAFETY: GetDpiForSystem takes no parameters and always succeeds on Win10+.
    let v = unsafe { GetDpiForSystem() };
    if v == 0 {
        BASE_DPI
    } else {
        v
    }
}
