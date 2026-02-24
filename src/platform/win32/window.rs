// ── Main window ───────────────────────────────────────────────────────────────
//
// Responsibilities in this file (unsafe confined here):
//   • Register the main window class.
//   • Create the top-level window and attach a menu bar.
//   • Run the Win32 message loop.
//   • Dispatch WM_COMMAND, WM_CLOSE, WM_DESTROY, WM_CREATE, WM_SIZE.
//   • Expose a safe error-dialog helper for use by main().
//
// Phase 2b will add:
//   • WM_CREATE: load SciLexer.dll, create Scintilla child + status bar.
//   • WM_SIZE: layout child controls to fill the client area.

#![allow(unsafe_code)]

use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{GetLastError, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, WPARAM},
        Graphics::Gdi::{GetStockObject, HBRUSH, WHITE_BRUSH},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            AppendMenuW, CreateMenu, CreateWindowExW, DefWindowProcW, DestroyWindow,
            DispatchMessageW, GetMessage, LoadCursorW, LoadIconW, MessageBoxW,
            PostQuitMessage, RegisterClassExW, SetMenu, ShowWindow, TranslateMessage,
            UpdateWindow, CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, IDC_ARROW,
            IDI_APPLICATION, MB_ICONERROR, MB_OK, MF_GRAYED, MF_POPUP, MF_STRING,
            MSG, SW_SHOW, WNDCLASSEXW, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY,
            WM_SIZE, WS_OVERLAPPEDWINDOW, HMENU,
        },
    },
};

use crate::error::{Result, RivetError};

// ── Window identity ───────────────────────────────────────────────────────────

/// Atom name used to register (and later find) the main window class.
const CLASS_NAME: PCWSTR = w!("RivetMainWindow");

/// Title bar text.
const APP_TITLE: PCWSTR = w!("Rivet");

/// Default client width in device pixels (before DPI scaling in Phase 8).
const DEFAULT_WIDTH: i32 = 960;

/// Default client height in device pixels.
const DEFAULT_HEIGHT: i32 = 640;

// ── Menu command IDs ──────────────────────────────────────────────────────────

const IDM_FILE_EXIT: usize = 1001;
const IDM_HELP_ABOUT: usize = 9001;

// ── Public API ────────────────────────────────────────────────────────────────

/// Register the main window class, create the window, and drive the message
/// loop until the user closes the application.
///
/// Records a startup timestamp and logs elapsed time (debug builds only) once
/// the window is first shown on screen.
pub(crate) fn run() -> Result<()> {
    // Startup benchmark harness — only compiled in debug builds so the
    // variable is never unused in release mode.
    #[cfg(debug_assertions)]
    let t0 = std::time::Instant::now();

    // SAFETY: GetModuleHandleW(None) returns the .exe's own HMODULE, which is
    // always valid for the process lifetime and never fails in practice.
    let hmodule = unsafe { GetModuleHandleW(None) }.map_err(RivetError::from)?;

    // HINSTANCE and HMODULE represent the same underlying value on Windows
    // (guaranteed by the Win32 ABI).  In windows-crate >=0.52 HINSTANCE is a
    // type alias for HMODULE; we use the explicit field conversion so the code
    // compiles regardless of whether they are distinct types.
    let hinstance = HINSTANCE(hmodule.0);

    register_class(hinstance)?;
    let hwnd = create_window(hinstance)?;

    // SAFETY: hwnd was just returned by CreateWindowExW and is valid.
    // ShowWindow returns the previous visibility state; UpdateWindow returns
    // a success BOOL — both are intentionally ignored here.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }

    // Startup milestone — window is now visible on screen.
    #[cfg(debug_assertions)]
    eprintln!("[rivet] window visible in {:.1} ms", t0.elapsed().as_secs_f64() * 1000.0);

    message_loop()
}

/// Show a modal error dialog with the given message.
///
/// Safe to call from any context; performs the UTF-16 conversion internally.
/// Used by `main()` when `run()` returns an error.
pub(crate) fn show_error_dialog(message: &str) {
    let msg_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let title_wide: Vec<u16> = "Rivet — Fatal Error"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: msg_wide and title_wide are valid null-terminated UTF-16 strings
    // that remain allocated for the duration of the MessageBoxW call.
    // HWND::default() (null) means the dialog has no owner window.
    // Return value (button pressed) is intentionally unused for an error dialog.
    unsafe {
        let _ = MessageBoxW(
            HWND::default(),
            PCWSTR(msg_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

// ── Window class registration ─────────────────────────────────────────────────

fn register_class(hinstance: HINSTANCE) -> Result<()> {
    // SAFETY: LoadIconW with IDI_APPLICATION always succeeds; it loads the
    // built-in application icon resource, which exists on all Windows versions.
    let icon = unsafe { LoadIconW(None, IDI_APPLICATION) }
        .map_err(RivetError::from)?;

    // SAFETY: LoadCursorW with IDC_ARROW always succeeds; the arrow cursor is
    // a built-in resource guaranteed to exist on all Windows versions.
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }
        .map_err(RivetError::from)?;

    // SAFETY: GetStockObject with WHITE_BRUSH always returns a valid HGDIOBJ.
    // Casting to HBRUSH is correct: stock brush objects are compatible types.
    let bg_brush = unsafe { HBRUSH(GetStockObject(WHITE_BRUSH).0) };

    let wndclass = WNDCLASSEXW {
        // WNDCLASSEXW is ~72 bytes; the cast to u32 is always lossless.
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        // CS_HREDRAW | CS_VREDRAW: repaint on resize.
        // Phase 2b may remove these once Scintilla fills the client area.
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(wnd_proc),
        cbClsExtra: 0,
        cbWndExtra: 0,
        hInstance: hinstance,
        hIcon: icon,
        hCursor: cursor,
        hbrBackground: bg_brush,
        lpszMenuName: PCWSTR::null(),
        lpszClassName: CLASS_NAME,
        hIconSm: icon,
    };

    // SAFETY: wndclass is fully initialised with valid handles;
    // CLASS_NAME is a valid null-terminated UTF-16 string literal.
    let atom = unsafe { RegisterClassExW(&wndclass) };
    if atom == 0 {
        return Err(last_error("RegisterClassExW"));
    }

    Ok(())
}

// ── Window creation ───────────────────────────────────────────────────────────

fn create_window(hinstance: HINSTANCE) -> Result<HWND> {
    // SAFETY: CLASS_NAME was just registered; hinstance is the exe's module.
    // HWND::default() (null parent) creates a top-level window.
    // HMENU::default() (null menu) — we attach the menu separately below.
    // None for lpParam: no creation data needed at this stage.
    let hwnd = unsafe {
        CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WINDOW_EX_STYLE(0),
            CLASS_NAME,
            APP_TITLE,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            DEFAULT_WIDTH,
            DEFAULT_HEIGHT,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
    };

    if hwnd == HWND::default() {
        return Err(last_error("CreateWindowExW"));
    }

    // Build and attach the menu bar.
    let menu = build_menu()?;
    // SAFETY: hwnd and menu are valid handles.
    unsafe { SetMenu(hwnd, menu) }.map_err(RivetError::from)?;

    Ok(hwnd)
}

// ── Menu construction ─────────────────────────────────────────────────────────

fn build_menu() -> Result<HMENU> {
    // SAFETY: CreateMenu has no preconditions; it always succeeds unless the
    // system is critically low on resources, in which case ? propagates the error.
    unsafe {
        let bar = CreateMenu().map_err(RivetError::from)?;

        // ── File ──────────────────────────────────────────────────────────────
        let file = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_EXIT, w!("E&xit\tAlt+F4"))
            .map_err(RivetError::from)?;

        // ── Edit ──────────────────────────────────────────────────────────────
        // Populated Phase 5; grayed stubs keep the menu non-empty.
        let edit = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING | MF_GRAYED, 0, w!("&Undo\tCtrl+Z"))
            .map_err(RivetError::from)?;

        // ── View ──────────────────────────────────────────────────────────────
        let view = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(view, MF_STRING | MF_GRAYED, 0, w!("Word &Wrap"))
            .map_err(RivetError::from)?;

        // ── Help ──────────────────────────────────────────────────────────────
        let help = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(help, MF_STRING, IDM_HELP_ABOUT, w!("&About Rivet…"))
            .map_err(RivetError::from)?;

        // Attach drop-downs to the menu bar.
        // The uIDNewItem parameter for MF_POPUP is the child HMENU cast to usize.
        AppendMenuW(bar, MF_POPUP, file.0 as usize, w!("&File"))
            .map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, edit.0 as usize, w!("&Edit"))
            .map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, view.0 as usize, w!("&View"))
            .map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, help.0 as usize, w!("&Help"))
            .map_err(RivetError::from)?;

        Ok(bar)
    }
}

// ── Message loop ──────────────────────────────────────────────────────────────

fn message_loop() -> Result<()> {
    let mut msg = MSG::default();

    loop {
        // SAFETY: &mut msg is a valid MSG pointer; HWND::default() retrieves
        // messages for all windows on this thread; 0,0 filter accepts all.
        let ret = unsafe { GetMessage(&mut msg, HWND::default(), 0, 0) };

        match ret.0 {
            // GetMessage returns -1 on error.
            -1 => return Err(last_error("GetMessage")),
            // Returns 0 when WM_QUIT is retrieved — exit the loop cleanly.
            0 => break,
            // Any other value: a normal message to dispatch.
            _ => unsafe {
                // SAFETY: msg was populated by a successful GetMessage call.
                // TranslateMessage return value (whether it generated WM_CHAR)
                // and DispatchMessageW's LRESULT are intentionally unused.
                let _ = TranslateMessage(&msg);
                let _ = DispatchMessageW(&msg);
            },
        }
    }

    Ok(())
}

// ── Window procedure ──────────────────────────────────────────────────────────

// SAFETY: wnd_proc is registered as lpfnWndProc in WNDCLASSEXW.
// Windows guarantees that hwnd, msg, wparam, and lparam are valid for the
// lifetime of this call; we must not store hwnd beyond the message handler.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // ── Lifecycle ─────────────────────────────────────────────────────────
        WM_CREATE => {
            // Phase 2b: load SciLexer.dll, create Scintilla child window and
            // status bar here.
            LRESULT(0)
        }

        WM_CLOSE => {
            // SAFETY: hwnd is the window being closed; DestroyWindow triggers
            // WM_DESTROY, which posts WM_QUIT via PostQuitMessage.
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            // SAFETY: PostQuitMessage with exit code 0 is always safe to call
            // from WM_DESTROY. It posts WM_QUIT to the thread's message queue.
            PostQuitMessage(0);
            LRESULT(0)
        }

        // ── Layout ────────────────────────────────────────────────────────────
        WM_SIZE => {
            // Phase 2b: resize Scintilla and status bar to fill the client area.
            // lparam low word = new client width, high word = new client height.
            let _new_width = lparam.0 & 0xFFFF;
            let _new_height = (lparam.0 >> 16) & 0xFFFF;
            LRESULT(0)
        }

        // ── Commands ──────────────────────────────────────────────────────────
        WM_COMMAND => {
            // Low word of WPARAM is the command identifier.
            let cmd_id = wparam.0 & 0xFFFF;

            match cmd_id {
                IDM_FILE_EXIT => {
                    // SAFETY: same as WM_CLOSE handler.
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }

                IDM_HELP_ABOUT => {
                    about_dialog(hwnd);
                    LRESULT(0)
                }

                _ => DefWindowProcW(hwnd, msg, wparam, lparam),
            }
        }

        // Default processing for all unhandled messages.
        // SAFETY: hwnd and message parameters are valid — provided by Windows.
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ── Helper dialogs ────────────────────────────────────────────────────────────

/// Display the "About Rivet" information dialog.
fn about_dialog(hwnd: HWND) {
    let body = concat!(
        "Rivet 0.1.0\n\n",
        "A simple, fast, and correct text editor for Windows 10/11.\n\n",
        "Licensed under MIT OR Apache-2.0.",
    );
    let body_wide: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();

    // SAFETY: body_wide is a valid null-terminated UTF-16 string that remains
    // allocated for the duration of the MessageBoxW call.
    // hwnd is the owner window from WndProc — valid for this call.
    // Return value (button pressed) is intentionally unused for an informational dialog.
    unsafe {
        let _ = MessageBoxW(hwnd, PCWSTR(body_wide.as_ptr()), w!("About Rivet"), MB_OK);
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

/// Capture the current Win32 last-error code and wrap it in a `RivetError`.
///
/// Call immediately after a Win32 function that signals failure — `GetLastError`
/// reads thread-local state that can be overwritten by any subsequent API call.
fn last_error(function: &'static str) -> RivetError {
    // SAFETY: GetLastError reads thread-local state set by the last Win32 call.
    // It is always safe to call and never fails.
    let code = unsafe { GetLastError() };
    RivetError::Win32 {
        function,
        code: code.0,
    }
}
