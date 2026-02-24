// ── Main window ───────────────────────────────────────────────────────────────
//
// Responsibilities:
//   • Register the main window class and create the top-level window.
//   • Attach a menu bar; run the Win32 message loop.
//   • WM_CREATE  → load SciLexer.dll + create Scintilla + tab bar + status bar.
//   • WM_SIZE    → resize children to fill the client area (three-zone layout).
//   • WM_DESTROY → drop WindowState (SciDll::drop calls FreeLibrary).
//   • WM_COMMAND → File > New/Open/Save/Save As/Exit, Help > About.
//   • WM_NOTIFY  → Scintilla notifications + TCN_SELCHANGE (tab switch).
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

const IDM_FILE_NEW: usize = 1000;
const IDM_FILE_OPEN: usize = 1001;
const IDM_FILE_SAVE: usize = 1002;
const IDM_FILE_SAVE_AS: usize = 1003;
const IDM_FILE_EXIT: usize = 1099;
const IDM_HELP_ABOUT: usize = 9001;

// ── Tab bar ───────────────────────────────────────────────────────────────────

/// Win32 window class for the common-controls tab control.
const TAB_CLASS: PCWSTR = w!("SysTabControl32");

/// Fixed height of the tab strip in device pixels.
/// Phase 8 will measure this dynamically for DPI awareness.
const TAB_BAR_HEIGHT: i32 = 25;

// Tab-control messages (from commctrl.h; windows crate 0.58 doesn't export them).
const TCM_FIRST: u32 = 0x1300;
const TCM_INSERTITEMW: u32 = TCM_FIRST + 7;  // 0x1307
const TCM_DELETEITEM: u32 = TCM_FIRST + 8;   // 0x1308  (used in Phase 4d)
const TCM_GETCURSEL: u32 = TCM_FIRST + 11;   // 0x130B
const TCM_SETCURSEL: u32 = TCM_FIRST + 12;   // 0x130C
const TCM_SETITEMW: u32 = TCM_FIRST + 61;    // 0x133D

// Tab-control notifications.
const TCN_SELCHANGE: u32 = 0xFFFF_FDD9; // (-551i32 as u32)

// Tab-control item flags / styles.
const TCIF_TEXT: u32 = 0x0001;

/// Portable Rust representation of the Win32 `TCITEMW` struct.
///
/// `#[repr(C)]` guarantees the layout matches what `SendMessageW(TCM_INSERTITEMW)`
/// expects.  The fields follow the C declaration order exactly.
#[repr(C)]
struct TCITEMW {
    mask:          u32,
    dw_state:      u32,
    dw_state_mask: u32,
    psz_text:      *mut u16,
    cch_text_max:  i32,
    i_image:       i32,
    l_param:       isize,
}

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
///   4. `hwnd_tab`, `hwnd_status` — HWND values only, no cleanup needed
struct WindowState {
    /// Top-level application state (documents, active tab index, …).
    app: App,
    /// One Scintilla child window per open tab; parallel to `app.tabs`.
    sci_views: Vec<ScintillaView>,
    /// RAII owner of `SciLexer.dll`; must outlive every `ScintillaView`.
    sci_dll: SciDll,
    /// The Win32 `SysTabControl32` tab strip child window.
    hwnd_tab: HWND,
    /// The Win32 `msctls_statusbar32` status bar child window.
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
    // ShowWindow / UpdateWindow return values are intentionally unused.
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
    let icon   = unsafe { LoadIconW(None, IDI_APPLICATION) }.map_err(RivetError::from)?;
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.map_err(RivetError::from)?;

    // SAFETY: GetStockObject(WHITE_BRUSH) always returns a valid HGDIOBJ.
    let bg_brush = unsafe { HBRUSH(GetStockObject(WHITE_BRUSH).0) };

    let wndclass = WNDCLASSEXW {
        cbSize:        std::mem::size_of::<WNDCLASSEXW>() as u32,
        style:         WNDCLASS_STYLES(0),
        lpfnWndProc:   Some(wnd_proc),
        cbClsExtra:    0,
        cbWndExtra:    0,
        hInstance:     hinstance,
        hIcon:         icon,
        hCursor:       cursor,
        hbrBackground: bg_brush,
        lpszMenuName:  PCWSTR::null(),
        lpszClassName: CLASS_NAME,
        hIconSm:       icon,
    };

    // SAFETY: wndclass is fully initialised with valid handles.
    let atom = unsafe { RegisterClassExW(&wndclass) };
    if atom == 0 { return Err(last_error("RegisterClassExW")); }
    Ok(())
}

fn create_window(hinstance: HINSTANCE) -> Result<HWND> {
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            APP_TITLE,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT, CW_USEDEFAULT,
            DEFAULT_WIDTH, DEFAULT_HEIGHT,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
    };

    if hwnd == HWND::default() { return Err(last_error("CreateWindowExW")); }

    let menu = build_menu()?;
    // SAFETY: hwnd and menu are valid handles.
    unsafe { SetMenu(hwnd, menu) }.map_err(RivetError::from)?;
    Ok(hwnd)
}

// ── Child-control creation ────────────────────────────────────────────────────

/// Create the tab bar, Scintilla editor, and status-bar children.
///
/// Called from WM_CREATE.  On failure the caller returns `LRESULT(-1)` to
/// abort window creation.
fn create_child_controls(hwnd_parent: HWND, hinstance: HINSTANCE) -> Result<WindowState> {
    // ── Scintilla DLL ─────────────────────────────────────────────────────────
    // Loading the DLL registers the "Scintilla" window class.
    let sci_dll = SciDll::load()?;

    // ── Tab bar ───────────────────────────────────────────────────────────────
    // Initial geometry (0,0,0,0); WM_SIZE will resize it correctly.
    // SAFETY: TAB_CLASS is a valid PCWSTR literal; hwnd_parent is valid.
    let hwnd_tab = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            TAB_CLASS,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS,
            0, 0, 0, 0,
            hwnd_parent,
            HMENU::default(),
            hinstance,
            None,
        )
    };
    if hwnd_tab == HWND::default() {
        return Err(last_error("CreateWindowExW (tab bar)"));
    }

    // ── Scintilla view (initial tab) ──────────────────────────────────────────
    let sci = ScintillaView::create(hwnd_parent, hinstance, &sci_dll)?;
    sci.show(true);
    let sci_views = vec![sci];

    // ── Status bar ────────────────────────────────────────────────────────────
    // SAFETY: STATUS_CLASS is valid; hwnd_parent and hinstance are valid.
    let hwnd_status = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            STATUS_CLASS,
            PCWSTR::null(),
            WS_CHILD | WS_VISIBLE | WS_CLIPSIBLINGS | WINDOW_STYLE(SBARS_SIZEGRIP),
            0, 0, 0, 0,
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

    // Split the status bar: encoding | EOL | caret position.
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

    // Insert the initial "Untitled" tab.
    // SAFETY: hwnd_tab is valid; "Untitled" is a valid string.
    unsafe { tab_insert(hwnd_tab, 0, "Untitled") };

    let state = WindowState { app, sci_views, sci_dll, hwnd_tab, hwnd_status };

    // SAFETY: all child HWNDs are valid; app has one initialised tab.
    unsafe { update_status_bar(&state) };
    Ok(state)
}

// ── Three-zone layout ─────────────────────────────────────────────────────────

/// Resize the tab bar, Scintilla view, and status bar to fill the client area.
///
/// Layout zones (top to bottom):
///   1. Tab strip  — `TAB_BAR_HEIGHT` px at top
///   2. Scintilla  — fills remaining space
///   3. Status bar — self-measures at bottom
///
/// # Safety
/// `state` must point to a live `WindowState` whose child HWNDs are valid.
unsafe fn layout_children(state: &WindowState, client_width: i32, client_height: i32) {
    // Zone 1: tab strip — full width, fixed height.
    let _ = SetWindowPos(
        state.hwnd_tab,
        HWND::default(),
        0, 0, client_width, TAB_BAR_HEIGHT,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );

    // Zone 3: status bar — self-repositions when it receives WM_SIZE.
    let _ = SendMessageW(state.hwnd_status, WM_SIZE, WPARAM(0), LPARAM(0));
    let mut sr = RECT::default();
    let _ = GetClientRect(state.hwnd_status, &mut sr);
    let status_h = sr.bottom;

    // Zone 2: Scintilla — fills the space between zones 1 and 3.
    let sci_y = TAB_BAR_HEIGHT;
    let sci_h = (client_height - TAB_BAR_HEIGHT - status_h).max(0);
    let _ = SetWindowPos(
        state.sci_views[state.app.active_idx].hwnd(),
        HWND::default(),
        0, sci_y, client_width, sci_h,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );
}

// ── Tab helpers ───────────────────────────────────────────────────────────────

/// Insert a new tab item at `idx` with the given `label`.
///
/// # Safety
/// `hwnd_tab` must be a valid `SysTabControl32` HWND.
unsafe fn tab_insert(hwnd_tab: HWND, idx: usize, label: &str) {
    let mut wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
    let mut item = TCITEMW {
        mask:          TCIF_TEXT,
        dw_state:      0,
        dw_state_mask: 0,
        psz_text:      wide.as_mut_ptr(),
        cch_text_max:  wide.len() as i32,
        i_image:       -1,
        l_param:       0,
    };
    // SAFETY: item is valid for the duration of the SendMessageW call;
    // wide provides the null-terminated UTF-16 text buffer.
    let _ = SendMessageW(
        hwnd_tab,
        TCM_INSERTITEMW,
        WPARAM(idx),
        LPARAM(&mut item as *mut TCITEMW as isize),
    );
}

/// Update the text of an existing tab at `idx`.
///
/// # Safety
/// `hwnd_tab` must be a valid `SysTabControl32` HWND.
unsafe fn tab_set_label(hwnd_tab: HWND, idx: usize, label: &str) {
    let mut wide: Vec<u16> = label.encode_utf16().chain(std::iter::once(0)).collect();
    let mut item = TCITEMW {
        mask:          TCIF_TEXT,
        dw_state:      0,
        dw_state_mask: 0,
        psz_text:      wide.as_mut_ptr(),
        cch_text_max:  wide.len() as i32,
        i_image:       -1,
        l_param:       0,
    };
    // SAFETY: see tab_insert.
    let _ = SendMessageW(
        hwnd_tab,
        TCM_SETITEMW,
        WPARAM(idx),
        LPARAM(&mut item as *mut TCITEMW as isize),
    );
}

/// Refresh the tab strip label for `idx` from the current document state.
///
/// # Safety
/// `state.hwnd_tab` must be a valid `SysTabControl32` HWND.
unsafe fn sync_tab_label(state: &WindowState, idx: usize) {
    let label = crate::ui::tabs::tab_label(&state.app.tabs[idx]);
    tab_set_label(state.hwnd_tab, idx, &label);
}

// ── Menu ──────────────────────────────────────────────────────────────────────

fn build_menu() -> Result<HMENU> {
    // SAFETY: CreateMenu / AppendMenuW are always safe on Win32 threads.
    unsafe {
        let bar = CreateMenu().map_err(RivetError::from)?;

        // File
        let file = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_NEW, w!("&New\tCtrl+N"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null())
            .map_err(RivetError::from)?;
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
        AppendMenuW(bar, MF_POPUP, file.0 as usize, w!("&File")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, edit.0 as usize, w!("&Edit")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, view.0 as usize, w!("&View")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, help.0 as usize, w!("&Help")).map_err(RivetError::from)?;

        Ok(bar)
    }
}

// ── Accelerator table ─────────────────────────────────────────────────────────

fn create_accelerators() -> Result<HACCEL> {
    let ctrl_virt: ACCEL_VIRT_FLAGS = FCONTROL | FVIRTKEY;
    let accels = [
        ACCEL { fVirt: ctrl_virt, key: b'N' as u16, cmd: IDM_FILE_NEW  as u16 },
        ACCEL { fVirt: ctrl_virt, key: b'O' as u16, cmd: IDM_FILE_OPEN as u16 },
        ACCEL { fVirt: ctrl_virt, key: b'S' as u16, cmd: IDM_FILE_SAVE as u16 },
    ];

    // SAFETY: accels is a valid, non-empty slice of ACCEL entries.
    let haccel = unsafe { CreateAcceleratorTableW(&accels) }.map_err(RivetError::from)?;
    Ok(haccel)
}

// ── Message loop ──────────────────────────────────────────────────────────────

fn message_loop(hwnd: HWND, haccel: HACCEL) -> Result<()> {
    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessage(&mut msg, HWND::default(), 0, 0) };
        match ret.0 {
            -1 => return Err(last_error("GetMessage")),
            0  => break,
            _  => unsafe {
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

// SAFETY: registered as `lpfnWndProc`; Windows guarantees the args are valid.
unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        // ── Startup ───────────────────────────────────────────────────────────
        WM_CREATE => {
            let hmodule = match GetModuleHandleW(None) {
                Ok(h)  => h,
                Err(_) => return LRESULT(-1),
            };
            let hinstance = HINSTANCE(hmodule.0);

            match create_child_controls(hwnd, hinstance) {
                Ok(state) => {
                    let ptr = Box::into_raw(Box::new(state));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
                    LRESULT(0)
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    eprintln!("[rivet] WM_CREATE failed: {e}");
                    let _ = e;
                    LRESULT(-1)
                }
            }
        }

        // ── Layout ────────────────────────────────────────────────────────────
        WM_SIZE => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                let new_w = (lparam.0 & 0xFFFF) as i32;
                let new_h = ((lparam.0 >> 16) & 0xFFFF) as i32;
                layout_children(&*ptr, new_w, new_h);
            }
            LRESULT(0)
        }

        // ── Teardown ──────────────────────────────────────────────────────────
        WM_CLOSE => {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() && (*ptr).app.active_doc().dirty {
                if !confirm_discard(hwnd) {
                    return LRESULT(0);
                }
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            // Drop order: app → sci_views → sci_dll (FreeLibrary) → hwnd_*.
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                drop(Box::from_raw(ptr));
            }
            PostQuitMessage(0);
            LRESULT(0)
        }

        // ── Commands ──────────────────────────────────────────────────────────
        WM_COMMAND => {
            let cmd = wparam.0 & 0xFFFF;
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            match cmd {
                IDM_FILE_NEW => {
                    if !ptr.is_null() { handle_new_file(hwnd, &mut *ptr); }
                    LRESULT(0)
                }
                IDM_FILE_OPEN => {
                    if !ptr.is_null() { handle_file_open(hwnd, &mut *ptr); }
                    LRESULT(0)
                }
                IDM_FILE_SAVE => {
                    if !ptr.is_null() { handle_file_save(hwnd, &mut *ptr, false); }
                    LRESULT(0)
                }
                IDM_FILE_SAVE_AS => {
                    if !ptr.is_null() { handle_file_save(hwnd, &mut *ptr, true); }
                    LRESULT(0)
                }
                IDM_FILE_EXIT => {
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

        // ── Scintilla + tab notifications ─────────────────────────────────────
        WM_NOTIFY => {
            // SAFETY: LPARAM is a pointer to NMHDR (or a struct beginning with
            // NMHDR) — guaranteed for all WM_NOTIFY messages.
            let hdr = &*(lparam.0 as *const windows::Win32::UI::Controls::NMHDR);
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if ptr.is_null() { return LRESULT(0); }

            match hdr.code {
                // ── Tab-control ───────────────────────────────────────────────
                TCN_SELCHANGE => {
                    // The tab control has already changed the selection; read it.
                    let sel = SendMessageW(
                        (*ptr).hwnd_tab, TCM_GETCURSEL, WPARAM(0), LPARAM(0),
                    );
                    if sel.0 < 0 { return LRESULT(0); } // shouldn't happen
                    let new_idx = sel.0 as usize;

                    if new_idx != (*ptr).app.active_idx {
                        // Hide the outgoing view, switch, show the incoming view.
                        (*ptr).sci_views[(*ptr).app.active_idx].show(false);
                        (*ptr).app.active_idx = new_idx;
                        (*ptr).sci_views[new_idx].show(true);

                        // Sync EOL from the newly-visible view.
                        let eol = (*ptr).sci_views[new_idx].eol_mode();
                        (*ptr).app.active_doc_mut().eol = eol;

                        // Resize the newly-visible Scintilla to fill its zone.
                        let mut rc = RECT::default();
                        let _ = GetClientRect(hwnd, &mut rc);
                        layout_children(&*ptr, rc.right, rc.bottom);

                        update_window_title(hwnd, &(*ptr).app);
                        update_status_bar(&*ptr);
                    }
                }

                // ── Scintilla — dirty tracking ─────────────────────────────────
                SCN_SAVEPOINTLEFT => {
                    (*ptr).app.active_doc_mut().dirty = true;
                    let idx = (*ptr).app.active_idx;
                    sync_tab_label(&*ptr, idx);
                    update_window_title(hwnd, &(*ptr).app);
                }
                SCN_SAVEPOINTREACHED => {
                    (*ptr).app.active_doc_mut().dirty = false;
                    let idx = (*ptr).app.active_idx;
                    sync_tab_label(&*ptr, idx);
                    update_window_title(hwnd, &(*ptr).app);
                }

                // ── Scintilla — caret moved ────────────────────────────────────
                SCN_UPDATEUI => {
                    let idx = (*ptr).app.active_idx;
                    let eol = (*ptr).sci_views[idx].eol_mode();
                    (*ptr).app.active_doc_mut().eol = eol;
                    update_status_bar(&*ptr);
                }

                _ => {}
            }
            LRESULT(0)
        }

        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

// ── New file ──────────────────────────────────────────────────────────────────

/// Handle File > New: open a fresh untitled tab.
///
/// If the active tab is already a clean untitled document, this is a no-op
/// (nothing to open; Ctrl+N pressed on an already-empty tab).
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_new_file(hwnd: HWND, state: &mut WindowState) {
    // Already a clean untitled tab — nothing to do.
    if state.app.active_doc().path.is_none() && !state.app.active_doc().dirty {
        return;
    }
    open_untitled_tab(hwnd, state);
}

// ── File open ─────────────────────────────────────────────────────────────────

/// Handle File > Open: show dialog, read file, load into a tab.
///
/// If the chosen file is already open in another tab, that tab is activated
/// instead of opening a duplicate.  If the current tab is a clean untitled
/// document the file is loaded into it; otherwise a new tab is created.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_file_open(hwnd: HWND, state: &mut WindowState) {
    let Some(path) = show_open_dialog(hwnd) else { return; };

    // Activate the existing tab if this file is already open.
    if let Some(dup_idx) = state
        .app
        .tabs
        .iter()
        .position(|t| t.path.as_deref() == Some(path.as_path()))
    {
        if dup_idx != state.app.active_idx {
            state.sci_views[state.app.active_idx].show(false);
            state.app.active_idx = dup_idx;
            state.sci_views[dup_idx].show(true);
            let _ = SendMessageW(
                state.hwnd_tab, TCM_SETCURSEL, WPARAM(dup_idx), LPARAM(0),
            );
            let eol = state.sci_views[dup_idx].eol_mode();
            state.app.active_doc_mut().eol = eol;
            let mut rc = RECT::default();
            let _ = GetClientRect(hwnd, &mut rc);
            layout_children(state, rc.right, rc.bottom);
            update_window_title(hwnd, &state.app);
            update_status_bar(state);
        }
        return;
    }

    let bytes = match std::fs::read(&path) {
        Ok(b)  => b,
        Err(e) => { show_error_dialog(&format!("Could not open file:\n{e}")); return; }
    };

    // Reuse the current tab if it is a clean untitled document.
    if state.app.active_doc().path.is_none() && !state.app.active_doc().dirty {
        load_file_into_active_tab(hwnd, state, path, &bytes);
    } else {
        open_file_in_new_tab(hwnd, state, path, &bytes);
    }
}

/// Load `path` / `bytes` into the currently active tab (which must be untitled
/// and clean before this call).
///
/// # Safety
/// `state` must be valid; the active tab must be untitled and clean.
unsafe fn load_file_into_active_tab(
    hwnd: HWND,
    state: &mut WindowState,
    path: std::path::PathBuf,
    bytes: &[u8],
) {
    let utf8 = state.app.open_file(path, bytes);
    let idx = state.app.active_idx;
    let (large_file, eol) = {
        let doc = state.app.active_doc();
        (doc.large_file, doc.eol)
    };
    state.sci_views[idx].set_large_file_mode(large_file);
    state.sci_views[idx].set_eol_mode(eol);
    state.sci_views[idx].set_text(&utf8);
    state.sci_views[idx].set_save_point();
    sync_tab_label(state, idx);
    update_window_title(hwnd, &state.app);
    update_status_bar(state);
}

/// Create a new tab and open `path` / `bytes` in it.
///
/// # Safety
/// `state` must be valid; `hwnd` is the parent window handle.
unsafe fn open_file_in_new_tab(
    hwnd: HWND,
    state: &mut WindowState,
    path: std::path::PathBuf,
    bytes: &[u8],
) {
    let sci = match new_scintilla_view(hwnd, state) {
        Some(s) => s,
        None    => return,
    };

    // Hide current view, push the new tab.
    state.sci_views[state.app.active_idx].show(false);
    let new_idx = state.app.push_untitled();
    state.sci_views.push(sci);
    state.app.active_idx = new_idx;

    // Insert a placeholder tab label (updated below by sync_tab_label).
    tab_insert(state.hwnd_tab, new_idx, "Untitled");
    let _ = SendMessageW(state.hwnd_tab, TCM_SETCURSEL, WPARAM(new_idx), LPARAM(0));

    // Load the file and configure the new Scintilla view.
    let utf8 = state.app.open_file(path, bytes);
    let (large_file, eol) = {
        let doc = state.app.active_doc();
        (doc.large_file, doc.eol)
    };
    state.sci_views[new_idx].set_large_file_mode(large_file);
    state.sci_views[new_idx].set_eol_mode(eol);
    state.sci_views[new_idx].set_text(&utf8);
    state.sci_views[new_idx].set_save_point();

    sync_tab_label(state, new_idx);
    state.sci_views[new_idx].show(true);

    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    layout_children(state, rc.right, rc.bottom);

    update_window_title(hwnd, &state.app);
    update_status_bar(state);
}

/// Create a fresh untitled tab and make it active.
///
/// # Safety
/// `state` must be valid; `hwnd` is the parent window handle.
unsafe fn open_untitled_tab(hwnd: HWND, state: &mut WindowState) {
    let sci = match new_scintilla_view(hwnd, state) {
        Some(s) => s,
        None    => return,
    };

    state.sci_views[state.app.active_idx].show(false);
    let new_idx = state.app.push_untitled();
    state.sci_views.push(sci);
    state.app.active_idx = new_idx;

    tab_insert(state.hwnd_tab, new_idx, "Untitled");
    let _ = SendMessageW(state.hwnd_tab, TCM_SETCURSEL, WPARAM(new_idx), LPARAM(0));

    state.sci_views[new_idx].show(true);

    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    layout_children(state, rc.right, rc.bottom);

    update_window_title(hwnd, &state.app);
    update_status_bar(state);
}

/// Create a new `ScintillaView` parented to `hwnd`.
///
/// Returns `None` and shows an error dialog on failure.
///
/// # Safety
/// `state.sci_dll` must be live; `hwnd` must be the main window.
unsafe fn new_scintilla_view(hwnd: HWND, state: &WindowState) -> Option<ScintillaView> {
    let hmodule = match GetModuleHandleW(None) {
        Ok(h)  => h,
        Err(_) => return None,
    };
    let hinstance = HINSTANCE(hmodule.0);
    match ScintillaView::create(hwnd, hinstance, &state.sci_dll) {
        Ok(s)  => Some(s),
        Err(e) => {
            show_error_dialog(&format!("Could not create editor view:\n{e}"));
            None
        }
    }
}

// ── File save ─────────────────────────────────────────────────────────────────

/// Handle File > Save / Save As.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_file_save(hwnd: HWND, state: &mut WindowState, force_dialog: bool) {
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
            None    => return,
        }
    } else {
        state.app.active_doc().path.clone().unwrap()
    };

    let idx  = state.app.active_idx;
    let utf8 = state.sci_views[idx].get_text();
    match state.app.save(path, &utf8) {
        Ok(()) => {
            state.sci_views[idx].set_save_point();
            sync_tab_label(state, idx);
            update_window_title(hwnd, &state.app);
        }
        Err(e) => show_error_dialog(&format!("Could not save file:\n{e}")),
    }
}

// ── Status bar / title ────────────────────────────────────────────────────────

/// Refresh all three status-bar parts from the current `WindowState`.
///
/// Parts:  0 = encoding  |  1 = EOL mode  |  2 = Ln / Col
///
/// # Safety
/// `state.hwnd_status` and the active sci_view must be valid.
unsafe fn update_status_bar(state: &WindowState) {
    let idx = state.app.active_idx;
    let (line, col) = state.sci_views[idx].caret_line_col();
    let (enc, eol) = {
        let doc = state.app.active_doc();
        (doc.encoding.as_str().to_owned(), doc.eol.as_str().to_owned())
    };
    let texts: [String; 3] = [enc, eol, format!("Ln {line}, Col {col}")];
    for (i, text) in texts.iter().enumerate() {
        let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
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
    let _ = SetWindowTextW(hwnd, PCWSTR(wide.as_ptr()));
}

// ── Helper dialogs ────────────────────────────────────────────────────────────

/// Ask the user what to do with unsaved changes.
///
/// Returns `true` if the close should proceed (user chose "Discard"),
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
    result == IDYES
}

fn about_dialog(hwnd: HWND) {
    let body = concat!(
        "Rivet 0.1.0\n\n",
        "A simple, fast, and correct text editor for Windows 10/11.\n\n",
        "Licensed under MIT OR Apache-2.0.",
    );
    let body_wide: Vec<u16> = body.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        let _ = MessageBoxW(hwnd, PCWSTR(body_wide.as_ptr()), w!("About Rivet"), MB_OK);
    }
}

// ── Error helpers ─────────────────────────────────────────────────────────────

fn last_error(function: &'static str) -> RivetError {
    // SAFETY: GetLastError reads thread-local state set by the failing call.
    let code = unsafe { GetLastError() };
    RivetError::Win32 { function, code: code.0 }
}
