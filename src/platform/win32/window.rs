// ── Main window ───────────────────────────────────────────────────────────────
//
// Responsibilities:
//   • Register the main window class and create the top-level window.
//   • Attach a menu bar; run the Win32 message loop.
//   • WM_CREATE  → load SciLexer.dll + create Scintilla + status-bar children.
//   • WM_SIZE    → resize children to fill the client area.
//   • WM_DESTROY → drop WindowState (SciDll::drop calls FreeLibrary).
//   • WM_COMMAND → File > Open/Save/Save As/Exit, Help > About.
//   • WM_NOTIFY  → Scintilla notifications (SCN_SAVEPOINTLEFT, SCN_UPDATEUI).
//   • Expose a safe error-dialog helper for main().
//
// State threading: a `Box<WindowState>` is stored in GWLP_USERDATA.
// It is set in WM_CREATE, read in WM_SIZE/NOTIFY/COMMAND, freed in WM_DESTROY.
// All accesses happen on the single UI thread.

#![allow(unsafe_code)]

use windows::{
    core::{w, PCWSTR},
    Win32::{
        Foundation::{GetLastError, HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{GetStockObject, HBRUSH, WHITE_BRUSH},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::{
            AppendMenuW, CreateAcceleratorTableW, CreateMenu, CreateWindowExW, DefWindowProcW,
            DestroyWindow, DispatchMessageW, GetClientRect, GetMessage, GetWindowLongPtrW,
            LoadCursorW, LoadIconW, MessageBoxW, PostQuitMessage, RegisterClassExW, SendMessageW,
            SetMenu, SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow,
            TranslateAcceleratorW, TranslateMessage, UpdateWindow, ACCEL, ACCEL_VIRT_FLAGS,
            CW_USEDEFAULT, FCONTROL, FVIRTKEY, GWLP_USERDATA, HACCEL, IDC_ARROW, IDI_APPLICATION,
            IDYES, MB_ICONERROR, MB_ICONWARNING, MB_OK, MB_YESNOCANCEL, MF_GRAYED,
            MF_POPUP, MF_SEPARATOR, MF_STRING, MSG, SW_SHOW,
            SWP_NOACTIVATE, SWP_NOZORDER, WINDOW_EX_STYLE, WINDOW_STYLE, WNDCLASS_STYLES,
            WNDCLASSEXW, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_NOTIFY, WM_SIZE,
            WS_CHILD, WS_CLIPSIBLINGS, WS_OVERLAPPEDWINDOW, WS_VISIBLE, HMENU,
        },
    },
};

use crate::{
    app::App,
    editor::scintilla::{
        messages::{SCN_SAVEPOINTLEFT, SCN_SAVEPOINTREACHED, SCN_UPDATEUI},
        SciDll, ScintillaView,
    },
    error::{Result, RivetError},
    platform::win32::dialogs::{show_open_dialog, show_save_dialog},
};

// ── Window identity ───────────────────────────────────────────────────────────

const CLASS_NAME: PCWSTR = w!("RivetMainWindow");
const APP_TITLE: PCWSTR = w!("Rivet");

/// Default window width, device pixels (DPI scaling added in Phase 8).
const DEFAULT_WIDTH: i32 = 960;
/// Default window height, device pixels.
const DEFAULT_HEIGHT: i32 = 640;

// ── Menu command IDs ──────────────────────────────────────────────────────────

const IDM_FILE_OPEN: usize = 1001;
const IDM_FILE_SAVE: usize = 1002;
const IDM_FILE_SAVE_AS: usize = 1003;
const IDM_FILE_EXIT: usize = 1099;
const IDM_HELP_ABOUT: usize = 9001;

// ── Status bar ────────────────────────────────────────────────────────────────

/// Win32 window class for the common-controls status bar.
const STATUS_CLASS: PCWSTR = w!("msctls_statusbar32");

/// `SBARS_SIZEGRIP` — adds a resize grip at the bottom-right corner.
const SBARS_SIZEGRIP: u32 = 0x0100;

/// `SB_SETTEXT` message — sets the text of a status-bar part.
const SB_SETTEXT: u32 = 0x0401;

/// `SB_SETPARTS` message — sets the number of parts and their right-edge pixel
/// positions.  WPARAM = part count; LPARAM = pointer to i32 array of edges.
/// A right-edge of -1 means "extend to the end of the bar".
const SB_SETPARTS: u32 = 0x0404;

/// Width of the encoding part (e.g. "UTF-16 LE"), device pixels.
const SB_PART_ENCODING_W: i32 = 120;
/// Width of the EOL part (e.g. "CRLF"), device pixels.
const SB_PART_EOL_W: i32 = 60;

// ── Per-window state ──────────────────────────────────────────────────────────

/// Heap-allocated state stored in `GWLP_USERDATA` for the lifetime of the
/// main window (from WM_CREATE to WM_DESTROY, inclusive).
///
/// # Drop order
///
/// Rust drops struct fields in declaration order:
///   1. `app`       — pure Rust, no handles
///   2. `sci_views` — child HWNDs already destroyed by Windows before WM_DESTROY
///   3. `sci_dll`   — `FreeLibrary` fires here, safely after all views are gone
///   4. `hwnd_status` — just an HWND value, no special cleanup
struct WindowState {
    /// Top-level application state (documents, active tab index, …).
    app: App,
    /// One Scintilla child window per open tab; parallel to `app.tabs`.
    sci_views: Vec<ScintillaView>,
    /// RAII owner of `SciLexer.dll`; must outlive every `ScintillaView`.
    sci_dll: SciDll,
    /// The Win32 status bar child window.
    hwnd_status: HWND,
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Register the main window class, create the window, and run the message
/// loop.  Returns when the user closes the application.
///
/// Logs the startup time to stderr in debug builds.
pub(crate) fn run() -> Result<()> {
    #[cfg(debug_assertions)]
    let t0 = std::time::Instant::now();

    // SAFETY: GetModuleHandleW(None) always succeeds — it returns the exe's
    // own module handle and never fails in a normally-loaded process.
    let hmodule = unsafe { GetModuleHandleW(None) }.map_err(RivetError::from)?;

    // HINSTANCE and HMODULE represent the same Win32 value (guaranteed by the
    // ABI).  The explicit field conversion compiles regardless of whether the
    // windows crate version treats them as the same or distinct types.
    let hinstance = HINSTANCE(hmodule.0);

    register_class(hinstance)?;
    let hwnd = create_window(hinstance)?;
    let haccel = create_accelerators()?;

    // SAFETY: hwnd was returned by CreateWindowExW and is valid.
    // ShowWindow / UpdateWindow return values are intentionally unused
    // (previous-visibility state and a success BOOL, respectively).
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = UpdateWindow(hwnd);
    }

    #[cfg(debug_assertions)]
    eprintln!(
        "[rivet] window visible in {:.1} ms",
        t0.elapsed().as_secs_f64() * 1000.0
    );

    message_loop(hwnd, haccel)
}

/// Show a modal "Fatal Error" dialog.  Safe to call from `main()`.
pub(crate) fn show_error_dialog(message: &str) {
    let msg_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let title_wide: Vec<u16> = "Rivet — Fatal Error"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    // SAFETY: both Vecs are valid null-terminated UTF-16 strings that outlive
    // this call.  HWND::default() (null) means no owner window.
    // Return value (button pressed) is intentionally unused.
    unsafe {
        let _ = MessageBoxW(
            HWND::default(),
            PCWSTR(msg_wide.as_ptr()),
            PCWSTR(title_wide.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

// ── Window class + creation ───────────────────────────────────────────────────

fn register_class(hinstance: HINSTANCE) -> Result<()> {
    // SAFETY: LoadIconW / LoadCursorW with the built-in system resource IDs
    // always succeed on all supported Windows versions.
    let icon = unsafe { LoadIconW(None, IDI_APPLICATION) }.map_err(RivetError::from)?;
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.map_err(RivetError::from)?;

    // SAFETY: GetStockObject(WHITE_BRUSH) always returns a valid HGDIOBJ.
    // Reinterpreting it as HBRUSH is correct — stock brush objects are
    // compatible with HBRUSH throughout the Win32 API.
    let bg_brush = unsafe { HBRUSH(GetStockObject(WHITE_BRUSH).0) };

    let wndclass = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
        // No CS_HREDRAW | CS_VREDRAW: Scintilla and the status bar fill the
        // entire client area, so a full-window repaint on resize causes flicker.
        style: WNDCLASS_STYLES(0),
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
    // CLASS_NAME is a static null-terminated UTF-16 literal.
    let atom = unsafe { RegisterClassExW(&wndclass) };
    if atom == 0 {
        return Err(last_error("RegisterClassExW"));
    }
    Ok(())
}

fn create_window(hinstance: HINSTANCE) -> Result<HWND> {
    // SAFETY: CLASS_NAME was registered above; hinstance is the exe's module.
    // HWND/HMENU::default() are null — correct for a top-level window with no
    // pre-attached menu (menu is attached separately via SetMenu below).
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
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

    let menu = build_menu()?;
    // SAFETY: hwnd and menu are valid handles just created above.
    unsafe { SetMenu(hwnd, menu) }.map_err(RivetError::from)?;

    Ok(hwnd)
}

// ── Child-control creation ────────────────────────────────────────────────────

/// Create the Scintilla editor and status-bar children.
///
/// Called from WM_CREATE.  On failure the caller returns `LRESULT(-1)` to
/// abort window creation, which causes `CreateWindowExW` to return null.
fn create_child_controls(hwnd_parent: HWND, hinstance: HINSTANCE) -> Result<WindowState> {
    // ── Scintilla DLL ─────────────────────────────────────────────────────────
    // Loading the DLL registers the "Scintilla" window class.
    let sci_dll = SciDll::load()?;

    // ── Scintilla view (initial tab) ──────────────────────────────────────────
    // New views are created hidden; show the first one immediately.
    let sci = ScintillaView::create(hwnd_parent, hinstance, &sci_dll)?;
    sci.show(true);
    let sci_views = vec![sci];

    // ── Status bar ────────────────────────────────────────────────────────────
    // `SBARS_SIZEGRIP` adds the resize grip at the bottom-right corner.
    // Initial position/size (0,0,0,0) — WM_SIZE will position it correctly.
    // SAFETY: STATUS_CLASS is a valid null-terminated class name (common
    // controls are registered by the OS).  hwnd_parent and hinstance are valid.
    let hwnd_status = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            STATUS_CLASS,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WINDOW_STYLE(SBARS_SIZEGRIP),
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

    if hwnd_status == HWND::default() {
        return Err(last_error("CreateWindowExW (status bar)"));
    }

    let app = App::new();

    // Split the status bar into three parts: encoding | EOL | caret position.
    // Right-edge positions in device pixels; the last entry is -1 (fill).
    let parts: [i32; 3] = [SB_PART_ENCODING_W, SB_PART_ENCODING_W + SB_PART_EOL_W, -1];
    // SAFETY: hwnd_status is valid; parts is a non-null i32 array.
    unsafe {
        let _ = SendMessageW(
            hwnd_status,
            SB_SETPARTS,
            WPARAM(parts.len()),
            LPARAM(parts.as_ptr() as isize),
        );
    }

    let state = WindowState { app, sci_views, sci_dll, hwnd_status };
    // SAFETY: hwnd_status and the active sci_view's hwnd() are valid children.
    unsafe { update_status_bar(&state) };
    Ok(state)
}

// ── Layout ────────────────────────────────────────────────────────────────────

/// Resize Scintilla and the status bar to fill the new client area.
///
/// # Safety
/// `state` must point to a live `WindowState` whose child HWNDs are valid
/// (i.e., between WM_CREATE and WM_DESTROY on the parent window).
unsafe fn layout_children(state: &WindowState, client_width: i32, client_height: i32) {
    // Notify the status bar of the new parent size; it repositions itself.
    // SAFETY: hwnd_status is a valid child window.
    let _ = SendMessageW(state.hwnd_status, WM_SIZE, WPARAM(0), LPARAM(0));

    // Measure the status bar's height after it has repositioned.
    let mut sr = RECT::default();
    // SAFETY: hwnd_status is valid; sr is a valid mutable RECT.
    let _ = GetClientRect(state.hwnd_status, &mut sr);
    let status_h = sr.bottom; // sr.top is always 0 for a client rect

    // Resize the active Scintilla view to fill the remaining area.
    // SAFETY: the active sci_view's hwnd() is the Scintilla child HWND.
    let _ = SetWindowPos(
        state.sci_views[state.app.active_idx].hwnd(),
        HWND::default(), // hWndInsertAfter ignored when SWP_NOZORDER is set
        0,
        0,
        client_width,
        (client_height - status_h).max(0),
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

// ── Menu ──────────────────────────────────────────────────────────────────────

fn build_menu() -> Result<HMENU> {
    // SAFETY: CreateMenu / AppendMenuW have no preconditions beyond running on
    // a Win32-enabled thread; they fail only under extreme resource pressure.
    unsafe {
        let bar = CreateMenu().map_err(RivetError::from)?;

        // File
        let file = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_OPEN, w!("&Open\u{2026}\tCtrl+O"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING | MF_GRAYED, IDM_FILE_SAVE, w!("&Save\tCtrl+S"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING | MF_GRAYED, IDM_FILE_SAVE_AS, w!("Save &As\u{2026}"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null())
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_EXIT, w!("E&xit\tAlt+F4"))
            .map_err(RivetError::from)?;

        // Edit  (populated Phase 5)
        let edit = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING | MF_GRAYED, 0, w!("&Undo\tCtrl+Z"))
            .map_err(RivetError::from)?;

        // View  (populated Phase 8)
        let view = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(view, MF_STRING | MF_GRAYED, 0, w!("Word &Wrap"))
            .map_err(RivetError::from)?;

        // Help
        let help = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(help, MF_STRING, IDM_HELP_ABOUT, w!("&About Rivet\u{2026}"))
            .map_err(RivetError::from)?;

        // Bar
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

// ── Accelerator table ─────────────────────────────────────────────────────────

/// Build the application-wide keyboard accelerator table.
///
/// Add one `ACCEL` entry per keyboard shortcut.  `cmd` must match the
/// corresponding `IDM_*` constant so that `TranslateAcceleratorW` synthesises
/// the right `WM_COMMAND` message.
fn create_accelerators() -> Result<HACCEL> {
    let ctrl_virt: ACCEL_VIRT_FLAGS = FCONTROL | FVIRTKEY;
    let accels = [
        ACCEL { fVirt: ctrl_virt, key: b'O' as u16, cmd: IDM_FILE_OPEN as u16 },
        ACCEL { fVirt: ctrl_virt, key: b'S' as u16, cmd: IDM_FILE_SAVE as u16 },
    ];

    // SAFETY: accels is a valid, non-empty slice of ACCEL entries.
    // CreateAcceleratorTableW copies the data into an OS-managed table.
    let haccel = unsafe { CreateAcceleratorTableW(&accels) }.map_err(RivetError::from)?;
    Ok(haccel)
}

// ── Message loop ──────────────────────────────────────────────────────────────

fn message_loop(hwnd: HWND, haccel: HACCEL) -> Result<()> {
    let mut msg = MSG::default();
    loop {
        // SAFETY: &mut msg is a valid MSG pointer.  HWND::default() (null)
        // retrieves messages for all windows on this thread; 0,0 accepts all.
        let ret = unsafe { GetMessage(&mut msg, HWND::default(), 0, 0) };

        match ret.0 {
            -1 => return Err(last_error("GetMessage")),
            0 => break,
            _ => unsafe {
                // SAFETY: msg was populated by a successful GetMessage call.
                // TranslateAcceleratorW checks for accelerator key combinations
                // and synthesises WM_COMMAND messages; if it handles the
                // message it returns non-zero and we skip normal translation.
                if TranslateAcceleratorW(hwnd, haccel, &msg) == 0 {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }
            },
        }
    }
    Ok(())
}

// ── Window procedure ──────────────────────────────────────────────────────────

// SAFETY: registered as `lpfnWndProc` in WNDCLASSEXW.  Windows guarantees
// that hwnd, msg, wparam, and lparam are valid for the duration of the call.
// We must not store hwnd beyond this handler's stack frame.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // ── Startup ───────────────────────────────────────────────────────────
        WM_CREATE => {
            // Retrieve HINSTANCE from the running exe so child windows share it.
            // SAFETY: GetModuleHandleW(None) always succeeds for a loaded exe.
            let hmodule = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(_) => return LRESULT(-1), // abort window creation
            };
            let hinstance = HINSTANCE(hmodule.0);

            match create_child_controls(hwnd, hinstance) {
                Ok(state) => {
                    let ptr = Box::into_raw(Box::new(state));
                    // SAFETY: ptr is a valid, aligned heap pointer; GWLP_USERDATA
                    // is the standard slot for per-window application data.
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
                    LRESULT(0)
                }
                Err(e) => {
                    // In debug builds, log the error; in release, stay silent
                    // (the app will fail visibly since CreateWindowExW returns null).
                    #[cfg(debug_assertions)]
                    eprintln!("[rivet] WM_CREATE failed: {e}");
                    let _ = e; // suppress unused-variable warning in release
                    LRESULT(-1) // abort window creation
                }
            }
        }

        // ── Layout ────────────────────────────────────────────────────────────
        WM_SIZE => {
            // SAFETY: GWLP_USERDATA holds a pointer set in WM_CREATE; null
            // check guards against any message arriving before WM_CREATE.
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                let new_w = (lparam.0 & 0xFFFF) as i32;
                let new_h = ((lparam.0 >> 16) & 0xFFFF) as i32;
                // SAFETY: ptr is a live Box<WindowState> raw pointer.
                layout_children(&*ptr, new_w, new_h);
            }
            LRESULT(0)
        }

        // ── Teardown ──────────────────────────────────────────────────────────
        WM_CLOSE => {
            // Guard against closing with unsaved changes on the active tab.
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() && (*ptr).app.active_doc().dirty {
                if !confirm_discard(hwnd) {
                    return LRESULT(0); // user chose Cancel — abort close
                }
            }
            // SAFETY: hwnd is the window being closed; DestroyWindow triggers
            // WM_DESTROY which posts WM_QUIT.
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            // Retrieve and drop the WindowState.
            // Drop order: app → sci_views → sci_dll (FreeLibrary) → hwnd_status.
            // sci_views' HWNDs are already invalid here (Windows destroyed child
            // windows when the parent was destroyed), but ScintillaView has no
            // Drop impl so that's fine.
            // SAFETY: GWLP_USERDATA holds the raw pointer from Box::into_raw
            // in WM_CREATE.  Clear it first to prevent re-entrancy.
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                // SAFETY: ptr is a live Box<WindowState>; this is the only place
                // it is reconstructed, and WM_DESTROY fires exactly once.
                drop(Box::from_raw(ptr));
            }
            // SAFETY: PostQuitMessage is always safe to call from WM_DESTROY.
            PostQuitMessage(0);
            LRESULT(0)
        }

        // ── Commands ──────────────────────────────────────────────────────────
        WM_COMMAND => {
            let cmd = wparam.0 & 0xFFFF;
            // SAFETY: GWLP_USERDATA holds Box<WindowState> set in WM_CREATE.
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            match cmd {
                IDM_FILE_OPEN => {
                    if !ptr.is_null() {
                        handle_file_open(hwnd, &mut *ptr);
                    }
                    LRESULT(0)
                }
                IDM_FILE_SAVE => {
                    if !ptr.is_null() {
                        handle_file_save(hwnd, &mut *ptr, false);
                    }
                    LRESULT(0)
                }
                IDM_FILE_SAVE_AS => {
                    if !ptr.is_null() {
                        handle_file_save(hwnd, &mut *ptr, true);
                    }
                    LRESULT(0)
                }
                IDM_FILE_EXIT => {
                    // SAFETY: see WM_CLOSE.
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

        // ── Scintilla notifications ────────────────────────────────────────────
        WM_NOTIFY => {
            // SAFETY: For WM_NOTIFY, LPARAM is a pointer to an NMHDR (or a
            // larger struct beginning with NMHDR).  We only read the `code`
            // field from the NMHDR prefix, which is always present.
            let hdr = &*(lparam.0 as *const windows::Win32::UI::Controls::NMHDR);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                match hdr.code {
                    SCN_SAVEPOINTLEFT => {
                        (*ptr).app.active_doc_mut().dirty = true;
                        update_window_title(hwnd, &(*ptr).app);
                    }
                    SCN_SAVEPOINTREACHED => {
                        (*ptr).app.active_doc_mut().dirty = false;
                        update_window_title(hwnd, &(*ptr).app);
                    }
                    SCN_UPDATEUI => {
                        // Caret moved or selection changed — refresh status bar.
                        // Also re-read EOL mode in case the user changed it.
                        let idx = (*ptr).app.active_idx;
                        let eol = (*ptr).sci_views[idx].eol_mode();
                        (*ptr).app.active_doc_mut().eol = eol;
                        update_status_bar(&*ptr);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }

        // SAFETY: hwnd and all message args are valid — provided by Windows.
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ── File operations ───────────────────────────────────────────────────────────

/// Handle File > Open: show dialog, read file, load into Scintilla.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_file_open(hwnd: HWND, state: &mut WindowState) {
    let Some(path) = show_open_dialog(hwnd) else {
        return; // user cancelled
    };

    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            let msg = format!("Could not open file:\n{e}");
            show_error_dialog(&msg);
            return;
        }
    };

    let utf8 = state.app.open_file(path, &bytes);

    // Configure the active Scintilla view for the document.
    let idx = state.app.active_idx;
    let doc = state.app.active_doc();
    state.sci_views[idx].set_large_file_mode(doc.large_file);
    state.sci_views[idx].set_eol_mode(doc.eol);
    state.sci_views[idx].set_text(&utf8);
    state.sci_views[idx].set_save_point();

    update_window_title(hwnd, &state.app);
}

/// Refresh all three status-bar parts from the current `WindowState`.
///
/// Parts:  0 = encoding  |  1 = EOL mode  |  2 = Ln / Col
///
/// # Safety
/// `state.hwnd_status` and the active sci_view's hwnd() must be valid.
unsafe fn update_status_bar(state: &WindowState) {
    let idx = state.app.active_idx;
    let (line, col) = state.sci_views[idx].caret_line_col();
    let doc = state.app.active_doc();
    let texts: [String; 3] = [
        doc.encoding.as_str().to_owned(),
        doc.eol.as_str().to_owned(),
        format!("Ln {line}, Col {col}"),
    ];
    for (i, text) in texts.iter().enumerate() {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: hwnd_status is valid; SB_SETTEXT with a valid part index and
        // null-terminated UTF-16 LPARAM is documented behaviour.
        let _ = SendMessageW(
            state.hwnd_status,
            SB_SETTEXT,
            WPARAM(i),
            LPARAM(wide.as_ptr() as isize),
        );
    }
}

/// Update the main window title from the current `App` state.
///
/// # Safety
/// `hwnd` must be the valid main-window handle.
unsafe fn update_window_title(hwnd: HWND, app: &App) {
    let title = app.window_title();
    let wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: hwnd is valid; wide is a null-terminated UTF-16 string.
    // Return value (BOOL success) is intentionally unused.
    let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
}

/// Handle File > Save (when `force_dialog` is false) or Save As (true).
///
/// If there is no current path, always opens the save dialog.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_file_save(hwnd: HWND, state: &mut WindowState, force_dialog: bool) {
    // Determine the save path.
    let path = if force_dialog || state.app.active_doc().path.is_none() {
        let default = state
            .app
            .active_doc()
            .path
            .as_deref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        match show_save_dialog(hwnd, &default) {
            Some(p) => p,
            None => return, // user cancelled
        }
    } else {
        state.app.active_doc().path.clone().unwrap()
    };

    let idx = state.app.active_idx;
    let utf8 = state.sci_views[idx].get_text();
    match state.app.save(path, &utf8) {
        Ok(()) => {
            state.sci_views[idx].set_save_point();
            update_window_title(hwnd, &state.app);
        }
        Err(e) => {
            let msg = format!("Could not save file:\n{e}");
            show_error_dialog(&msg);
        }
    }
}

/// Ask the user what to do with unsaved changes.
///
/// Returns `true` if the close should proceed (user chose "Don't Save"),
/// `false` if the user cancelled.
///
/// # Safety
/// `hwnd` must be a valid window handle.
unsafe fn confirm_discard(hwnd: HWND) -> bool {
    let text = "This document has unsaved changes.\n\nDo you want to discard them and close?";
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let result = MessageBoxW(
        hwnd,
        PCWSTR(wide.as_ptr()),
        w!("Rivet"),
        MB_YESNOCANCEL | MB_ICONWARNING,
    );
    // IDYES = discard and close, IDCANCEL = stay open, IDNO = stay open
    result == IDYES
}

// ── Helper dialogs ────────────────────────────────────────────────────────────

fn about_dialog(hwnd: HWND) {
    let body = concat!(
        "Rivet 0.1.0\n\n",
        "A simple, fast, and correct text editor for Windows 10/11.\n\n",
        "Licensed under MIT OR Apache-2.0.",
    );
    let body_wide: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();

    // SAFETY: body_wide is a valid null-terminated UTF-16 string that remains
    // allocated for the duration of the call.  hwnd is valid (from WndProc).
    // Return value (button pressed) is intentionally unused.
    unsafe {
        let _ = MessageBoxW(hwnd, PCWSTR(body_wide.as_ptr()), w!("About Rivet"), MB_OK);
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

/// Read the current Win32 last-error code.
///
/// **Must** be called immediately after a failing Win32 function — the
/// thread-local error state is overwritten by any subsequent API call.
fn last_error(function: &'static str) -> RivetError {
    // SAFETY: GetLastError reads thread-local state; it is always safe and
    // never fails.
    let code = unsafe { GetLastError() };
    RivetError::Win32 {
        function,
        code: code.0,
    }
}
