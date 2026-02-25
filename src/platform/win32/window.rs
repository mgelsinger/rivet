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
//   • WM_TIMER   → periodic 30-second session checkpoint.
//   • Expose a safe error-dialog helper for main().
//
// State threading: a `Box<WindowState>` is stored in GWLP_USERDATA.
// It is set in WM_CREATE, read in WM_SIZE/NOTIFY/COMMAND, freed in WM_DESTROY.
// All accesses happen on the single UI thread.

#![allow(unsafe_code)]
#![allow(dangerous_implicit_autorefs)]

use windows::{
    core::{w, PCWSTR, PWSTR},
    Win32::{
        Foundation::{GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::Gdi::{GetStockObject, UpdateWindow, HBRUSH, WHITE_BRUSH},
        System::{Diagnostics::Debug::MessageBeep, LibraryLoader::GetModuleHandleW},
        UI::{
            Controls::Dialogs::{FindTextW, ReplaceTextW, FINDREPLACEW, FINDREPLACE_FLAGS},
            WindowsAndMessaging::{
                AppendMenuW, CheckMenuItem, CreateAcceleratorTableW, CreateMenu, CreateWindowExW,
                DefWindowProcW, DestroyWindow, DialogBoxIndirectParamW, DispatchMessageW,
                EndDialog, GetClientRect, GetDlgItem, GetDlgItemTextW, GetMenu, GetMessageW,
                GetWindowLongPtrW, IsDialogMessageW, KillTimer, LoadCursorW, LoadIconW,
                MessageBoxW, PostQuitMessage, RegisterClassExW, RegisterWindowMessageW,
                SendMessageW, SetDlgItemTextW, SetForegroundWindow, SetMenu, SetTimer,
                SetWindowLongPtrW, SetWindowPos, SetWindowTextW, ShowWindow, TranslateAcceleratorW,
                TranslateMessage, ACCEL, ACCEL_VIRT_FLAGS, CW_USEDEFAULT, DLGTEMPLATE, FCONTROL,
                FSHIFT, FVIRTKEY, GWLP_USERDATA, HACCEL, HMENU, IDC_ARROW, IDI_APPLICATION, IDNO,
                IDYES, MB_ICONERROR, MB_ICONWARNING, MB_OK, MB_YESNO, MB_YESNOCANCEL,
                MESSAGEBOX_STYLE, MF_BYCOMMAND, MF_CHECKED, MF_POPUP, MF_SEPARATOR, MF_STRING,
                MF_UNCHECKED, MSG, SWP_NOACTIVATE, SWP_NOZORDER, SW_SHOW, WINDOW_EX_STYLE,
                WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_INITDIALOG,
                WM_NOTIFY, WM_SIZE, WM_TIMER, WNDCLASSEXW, WNDCLASS_STYLES, WS_CHILD,
                WS_CLIPSIBLINGS, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
            },
        },
    },
};

use crate::{
    app::{App, EolMode},
    editor::scintilla::{
        messages::{
            SCFIND_MATCHCASE, SCFIND_WHOLEWORD, SCN_SAVEPOINTLEFT, SCN_SAVEPOINTREACHED,
            SCN_UPDATEUI,
        },
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
const IDM_FILE_CLOSE: usize = 1004;
const IDM_FILE_EXIT: usize = 1099;

const IDM_EDIT_UNDO: usize = 2000;
const IDM_EDIT_REDO: usize = 2001;
const IDM_EDIT_CUT: usize = 2002;
const IDM_EDIT_COPY: usize = 2003;
const IDM_EDIT_PASTE: usize = 2004;
const IDM_EDIT_DELETE: usize = 2005;
const IDM_EDIT_SELECT_ALL: usize = 2006;

const IDM_FORMAT_EOL_CRLF: usize = 3000;
const IDM_FORMAT_EOL_LF: usize = 3001;
const IDM_FORMAT_EOL_CR: usize = 3002;

const IDM_VIEW_WORD_WRAP: usize = 4000;
const IDM_VIEW_DARK_MODE: usize = 4001;

const IDM_SEARCH_FIND: usize = 5000;
const IDM_SEARCH_REPLACE: usize = 5001;
const IDM_SEARCH_FIND_NEXT: usize = 5002;
const IDM_SEARCH_FIND_PREV: usize = 5003;
const IDM_SEARCH_GOTO_LINE: usize = 5004;

const IDM_HELP_ABOUT: usize = 9001;

// ── Auto-save timer ───────────────────────────────────────────────────────────

/// `nIDEvent` passed to `SetTimer` for the periodic session checkpoint.
const AUTOSAVE_TIMER_ID: usize = 1;
/// Auto-save interval in milliseconds (30 seconds).
const AUTOSAVE_INTERVAL_MS: u32 = 30_000;

// ── FindReplace dialog flags (from commdlg.h) ─────────────────────────────────

const FR_DOWN: u32 = 0x0001; // search direction: forward
const FR_WHOLEWORD: u32 = 0x0002;
const FR_MATCHCASE: u32 = 0x0004;
const FR_FINDNEXT: u32 = 0x0008;
const FR_REPLACE: u32 = 0x0010;
const FR_REPLACEALL: u32 = 0x0020;
const FR_DIALOGTERM: u32 = 0x0040;

/// Virtual key code for the F3 key (used in accelerator table).
const VK_F3: u16 = 0x72;

// ── Registered message ID for the modeless Find/Replace dialog ────────────────

/// Populated once in `run()` via `RegisterWindowMessageW("commdlg_FindReplace")`.
/// Every WM_* value dispatched through the message loop is compared against this
/// before the standard `match msg { … }` to intercept Find/Replace notifications.
static FIND_MSG_ID: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

// ── Tab bar ───────────────────────────────────────────────────────────────────

/// Win32 window class for the common-controls tab control.
const TAB_CLASS: PCWSTR = w!("SysTabControl32");

/// Baseline height of the tab strip at 96 DPI; scaled by actual DPI at runtime.
const TAB_BAR_BASE_H: i32 = 25;

/// `WM_DPICHANGED` — sent when the window moves to a monitor with a different DPI.
const WM_DPICHANGED: u32 = 0x02E0;

/// `DWMWA_USE_IMMERSIVE_DARK_MODE` attribute ID for `DwmSetWindowAttribute`.
const DWMWA_DARK_MODE: i32 = 20;

// Tab-control messages (from commctrl.h; windows crate 0.58 doesn't export them).
const TCM_FIRST: u32 = 0x1300;
const TCM_INSERTITEMW: u32 = TCM_FIRST + 7; // 0x1307
const TCM_DELETEITEM: u32 = TCM_FIRST + 8; // 0x1308  (used in Phase 4d)
const TCM_GETCURSEL: u32 = TCM_FIRST + 11; // 0x130B
const TCM_SETCURSEL: u32 = TCM_FIRST + 12; // 0x130C
const TCM_SETITEMW: u32 = TCM_FIRST + 61; // 0x133D

// Tab-control notifications.
const TCN_SELCHANGE: u32 = 0xFFFF_FDD9; // (-551i32 as u32)

// Tab-control item flags / styles.
const TCIF_TEXT: u32 = 0x0001;

/// Portable Rust representation of the Win32 `TCITEMW` struct.
///
/// `#[repr(C)]` guarantees the layout matches what `SendMessageW(TCM_INSERTITEMW)`
/// expects.  The fields follow the C declaration order exactly.
#[repr(C)]
#[allow(clippy::upper_case_acronyms)]
struct TCITEMW {
    mask: u32,
    dw_state: u32,
    dw_state_mask: u32,
    psz_text: *mut u16,
    cch_text_max: i32,
    i_image: i32,
    l_param: isize,
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

/// Width of the encoding part at 96 DPI baseline (e.g. "UTF-16 LE").
const SB_PART_ENCODING_W_BASE: i32 = 120;
/// Width of the EOL part at 96 DPI baseline (e.g. "CRLF").
const SB_PART_EOL_W_BASE: i32 = 60;
/// Width of the language part at 96 DPI baseline (e.g. "JavaScript").
const SB_PART_LANG_W_BASE: i32 = 130;

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
    // ── Phase 8: DPI + dark mode ───────────────────────────────────────────────
    /// Current display DPI; initialised to 96, updated in `post_create_init`
    /// and `WM_DPICHANGED`.
    dpi: u32,
    /// Whether dark mode is currently active; persisted in `session.json`.
    dark_mode: bool,
    // ── Phase 6: Find / Replace state ─────────────────────────────────────────
    /// Heap-stable UTF-16 buffer for the Find text (pointed to by `findreplace`).
    find_buf: Box<[u16; 512]>,
    /// Heap-stable UTF-16 buffer for the Replace text.
    #[allow(dead_code)]
    replace_buf: Box<[u16; 512]>,
    /// Shared `FINDREPLACEW` struct — passed to `FindTextW` / `ReplaceTextW`.
    /// Its `lpstrFindWhat` and `lpstrReplaceWith` pointers into the boxes above
    /// are stable because `WindowState` is never moved after `Box::into_raw`.
    findreplace: FINDREPLACEW,
    /// HWND of the open modeless Find (or Replace) dialog, or `HWND::default()`.
    hwnd_find_dlg: HWND,
}

// ── Public entry points ───────────────────────────────────────────────────────

/// Register the main window class, create the window, and run the message
/// loop.  Returns when the user closes the application.
///
/// Logs the startup time to stderr in debug builds.
pub(crate) fn run() -> Result<()> {
    #[cfg(debug_assertions)]
    let t0 = std::time::Instant::now();

    // Per-Monitor v2 DPI awareness — must be set before any window is created.
    crate::platform::win32::dpi::init();

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

    // Register the custom message that FindTextW / ReplaceTextW send to the
    // owner window.  The ID is process-unique and must be checked in wnd_proc
    // before the standard match on msg.
    // SAFETY: RegisterWindowMessageW is always safe; the literal is valid UTF-16.
    let find_msg = unsafe { RegisterWindowMessageW(w!("commdlg_FindReplace")) };
    if find_msg != 0 {
        let _ = FIND_MSG_ID.set(find_msg);
    }

    // Restore the previous session.
    // SAFETY: WM_CREATE (fired synchronously inside create_window) already
    // stored the Box<WindowState> in GWLP_USERDATA before we reach this point.
    unsafe {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
        if !ptr.is_null() {
            restore_session(hwnd, &mut *ptr);
        }
    }

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
    let icon = unsafe { LoadIconW(None, IDI_APPLICATION) }.map_err(RivetError::from)?;
    let cursor = unsafe { LoadCursorW(None, IDC_ARROW) }.map_err(RivetError::from)?;

    // SAFETY: GetStockObject(WHITE_BRUSH) always returns a valid HGDIOBJ.
    let bg_brush = unsafe { HBRUSH(GetStockObject(WHITE_BRUSH).0) };

    let wndclass = WNDCLASSEXW {
        cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
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

    // SAFETY: wndclass is fully initialised with valid handles.
    let atom = unsafe { RegisterClassExW(&wndclass) };
    if atom == 0 {
        return Err(last_error("RegisterClassExW"));
    }
    Ok(())
}

fn create_window(hinstance: HINSTANCE) -> Result<HWND> {
    // Scale the initial window size to the primary monitor's DPI so the window
    // appears at a consistent logical size on high-DPI displays.
    let sys_dpi = crate::platform::win32::dpi::get_system_dpi();
    let init_w = crate::platform::win32::dpi::scale(DEFAULT_WIDTH, sys_dpi);
    let init_h = crate::platform::win32::dpi::scale(DEFAULT_HEIGHT, sys_dpi);

    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS_NAME,
            APP_TITLE,
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            init_w,
            init_h,
            HWND::default(),
            HMENU::default(),
            hinstance,
            None,
        )
    }
    .map_err(RivetError::from)?;

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
            0,
            0,
            0,
            0,
            hwnd_parent,
            HMENU::default(),
            hinstance,
            None,
        )
    }
    .map_err(RivetError::from)?;

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
            0,
            0,
            0,
            0,
            hwnd_parent,
            HMENU::default(),
            hinstance,
            None,
        )
    }
    .map_err(RivetError::from)?;

    let app = App::new();

    // Split the status bar at 96 DPI baseline; `post_create_init` rescales if needed.
    let parts: [i32; 4] = [
        SB_PART_ENCODING_W_BASE,
        SB_PART_ENCODING_W_BASE + SB_PART_EOL_W_BASE,
        SB_PART_ENCODING_W_BASE + SB_PART_EOL_W_BASE + SB_PART_LANG_W_BASE,
        -1, // language: extends to fill remaining width
    ];
    // SAFETY: hwnd_status is valid; parts is a non-null i32 array of right-edge pixels.
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

    // ── Phase 6: Find/Replace buffers ─────────────────────────────────────────
    // The buffers are heap-allocated so their addresses are stable even after
    // WindowState is moved into Box::into_raw.  We capture the raw pointers
    // before moving ownership into the struct.
    let find_buf = Box::new([0u16; 512]);
    let replace_buf = Box::new([0u16; 512]);
    let find_ptr = find_buf.as_ptr() as *mut u16;
    let repl_ptr = replace_buf.as_ptr() as *mut u16;
    let findreplace = FINDREPLACEW {
        lStructSize: std::mem::size_of::<FINDREPLACEW>() as u32,
        hwndOwner: hwnd_parent,
        lpstrFindWhat: PWSTR(find_ptr),
        wFindWhatLen: 512,
        lpstrReplaceWith: PWSTR(repl_ptr),
        wReplaceWithLen: 512,
        Flags: FINDREPLACE_FLAGS(FR_DOWN),
        ..Default::default()
    };

    let state = WindowState {
        app,
        sci_views,
        sci_dll,
        hwnd_tab,
        hwnd_status,
        dpi: crate::platform::win32::dpi::BASE_DPI,
        dark_mode: false,
        find_buf,
        replace_buf,
        findreplace,
        hwnd_find_dlg: HWND::default(),
    };

    // SAFETY: all child HWNDs are valid; app has one initialised tab.
    unsafe { update_status_bar(&state) };
    Ok(state)
}

// ── Three-zone layout ─────────────────────────────────────────────────────────

/// Resize the tab bar, Scintilla view, and status bar to fill the client area.
///
/// Layout zones (top to bottom):
///   1. Tab strip  — `TAB_BAR_BASE_H` px at 96 DPI, scaled at runtime
///   2. Scintilla  — fills remaining space
///   3. Status bar — self-measures at bottom
///
/// # Safety
/// `state` must point to a live `WindowState` whose child HWNDs are valid.
unsafe fn layout_children(state: &WindowState, client_width: i32, client_height: i32) {
    let tab_h = crate::platform::win32::dpi::scale(TAB_BAR_BASE_H, state.dpi);

    // Zone 1: tab strip — full width, DPI-scaled height.
    let _ = SetWindowPos(
        state.hwnd_tab,
        HWND::default(),
        0,
        0,
        client_width,
        tab_h,
        SWP_NOZORDER | SWP_NOACTIVATE,
    );

    // Zone 3: status bar — self-repositions when it receives WM_SIZE.
    let _ = SendMessageW(state.hwnd_status, WM_SIZE, WPARAM(0), LPARAM(0));
    let mut sr = RECT::default();
    let _ = GetClientRect(state.hwnd_status, &mut sr);
    let status_h = sr.bottom;

    // Zone 2: Scintilla — fills the space between zones 1 and 3.
    let sci_y = tab_h;
    let sci_h = (client_height - tab_h - status_h).max(0);
    let _ = SetWindowPos(
        state.sci_views[state.app.active_idx].hwnd(),
        HWND::default(),
        0,
        sci_y,
        client_width,
        sci_h,
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
        mask: TCIF_TEXT,
        dw_state: 0,
        dw_state_mask: 0,
        psz_text: wide.as_mut_ptr(),
        cch_text_max: wide.len() as i32,
        i_image: -1,
        l_param: 0,
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
        mask: TCIF_TEXT,
        dw_state: 0,
        dw_state_mask: 0,
        psz_text: wide.as_mut_ptr(),
        cch_text_max: wide.len() as i32,
        i_image: -1,
        l_param: 0,
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

        // ── File ──────────────────────────────────────────────────────────────
        let file = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_NEW, w!("&New\tCtrl+N")).map_err(RivetError::from)?;
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null()).map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_OPEN, w!("&Open\u{2026}\tCtrl+O"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_SAVE, w!("&Save\tCtrl+S"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_SAVE_AS, w!("Save &As\u{2026}"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null()).map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_CLOSE, w!("&Close Tab\tCtrl+W"))
            .map_err(RivetError::from)?;
        AppendMenuW(file, MF_SEPARATOR, 0, PCWSTR::null()).map_err(RivetError::from)?;
        AppendMenuW(file, MF_STRING, IDM_FILE_EXIT, w!("E&xit\tAlt+F4"))
            .map_err(RivetError::from)?;

        // ── Edit ──────────────────────────────────────────────────────────────
        let edit = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING, IDM_EDIT_UNDO, w!("&Undo\tCtrl+Z"))
            .map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING, IDM_EDIT_REDO, w!("&Redo\tCtrl+Y"))
            .map_err(RivetError::from)?;
        AppendMenuW(edit, MF_SEPARATOR, 0, PCWSTR::null()).map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING, IDM_EDIT_CUT, w!("Cu&t\tCtrl+X")).map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING, IDM_EDIT_COPY, w!("&Copy\tCtrl+C"))
            .map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING, IDM_EDIT_PASTE, w!("&Paste\tCtrl+V"))
            .map_err(RivetError::from)?;
        AppendMenuW(edit, MF_STRING, IDM_EDIT_DELETE, w!("&Delete")).map_err(RivetError::from)?;
        AppendMenuW(edit, MF_SEPARATOR, 0, PCWSTR::null()).map_err(RivetError::from)?;
        AppendMenuW(
            edit,
            MF_STRING,
            IDM_EDIT_SELECT_ALL,
            w!("Select &All\tCtrl+A"),
        )
        .map_err(RivetError::from)?;

        // ── Format ────────────────────────────────────────────────────────────
        let format = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(
            format,
            MF_STRING,
            IDM_FORMAT_EOL_CRLF,
            w!("Convert to &Windows (CRLF)"),
        )
        .map_err(RivetError::from)?;
        AppendMenuW(
            format,
            MF_STRING,
            IDM_FORMAT_EOL_LF,
            w!("Convert to &Unix (LF)"),
        )
        .map_err(RivetError::from)?;
        AppendMenuW(
            format,
            MF_STRING,
            IDM_FORMAT_EOL_CR,
            w!("Convert to &Classic Mac (CR)"),
        )
        .map_err(RivetError::from)?;

        // ── Search ────────────────────────────────────────────────────────────
        let search = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(
            search,
            MF_STRING,
            IDM_SEARCH_FIND,
            w!("&Find\u{2026}\tCtrl+F"),
        )
        .map_err(RivetError::from)?;
        AppendMenuW(
            search,
            MF_STRING,
            IDM_SEARCH_REPLACE,
            w!("&Replace\u{2026}\tCtrl+H"),
        )
        .map_err(RivetError::from)?;
        AppendMenuW(
            search,
            MF_STRING,
            IDM_SEARCH_FIND_NEXT,
            w!("Find &Next\tF3"),
        )
        .map_err(RivetError::from)?;
        AppendMenuW(
            search,
            MF_STRING,
            IDM_SEARCH_FIND_PREV,
            w!("Find &Prev\tShift+F3"),
        )
        .map_err(RivetError::from)?;
        AppendMenuW(search, MF_SEPARATOR, 0, PCWSTR::null()).map_err(RivetError::from)?;
        AppendMenuW(
            search,
            MF_STRING,
            IDM_SEARCH_GOTO_LINE,
            w!("&Go to Line\u{2026}\tCtrl+G"),
        )
        .map_err(RivetError::from)?;

        // ── View ──────────────────────────────────────────────────────────────
        let view = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(view, MF_STRING, IDM_VIEW_WORD_WRAP, w!("Word &Wrap"))
            .map_err(RivetError::from)?;
        AppendMenuW(view, MF_SEPARATOR, 0, PCWSTR::null()).map_err(RivetError::from)?;
        AppendMenuW(view, MF_STRING, IDM_VIEW_DARK_MODE, w!("&Dark Mode"))
            .map_err(RivetError::from)?;

        // ── Help ──────────────────────────────────────────────────────────────
        let help = CreateMenu().map_err(RivetError::from)?;
        AppendMenuW(help, MF_STRING, IDM_HELP_ABOUT, w!("&About Rivet\u{2026}"))
            .map_err(RivetError::from)?;

        // ── Bar: File | Edit | Format | Search | View | Help ─────────────────
        AppendMenuW(bar, MF_POPUP, file.0 as usize, w!("&File")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, edit.0 as usize, w!("&Edit")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, format.0 as usize, w!("F&ormat")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, search.0 as usize, w!("&Search")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, view.0 as usize, w!("&View")).map_err(RivetError::from)?;
        AppendMenuW(bar, MF_POPUP, help.0 as usize, w!("&Help")).map_err(RivetError::from)?;

        Ok(bar)
    }
}

// ── Accelerator table ─────────────────────────────────────────────────────────

fn create_accelerators() -> Result<HACCEL> {
    let ctrl_virt: ACCEL_VIRT_FLAGS = FCONTROL | FVIRTKEY;
    let virt_only: ACCEL_VIRT_FLAGS = FVIRTKEY;
    let shift_virt: ACCEL_VIRT_FLAGS = FVIRTKEY | FSHIFT;
    let accels = [
        ACCEL {
            fVirt: ctrl_virt,
            key: b'N' as u16,
            cmd: IDM_FILE_NEW as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'O' as u16,
            cmd: IDM_FILE_OPEN as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'S' as u16,
            cmd: IDM_FILE_SAVE as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'W' as u16,
            cmd: IDM_FILE_CLOSE as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'Z' as u16,
            cmd: IDM_EDIT_UNDO as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'Y' as u16,
            cmd: IDM_EDIT_REDO as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'X' as u16,
            cmd: IDM_EDIT_CUT as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'C' as u16,
            cmd: IDM_EDIT_COPY as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'V' as u16,
            cmd: IDM_EDIT_PASTE as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'A' as u16,
            cmd: IDM_EDIT_SELECT_ALL as u16,
        },
        // Search
        ACCEL {
            fVirt: ctrl_virt,
            key: b'F' as u16,
            cmd: IDM_SEARCH_FIND as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'H' as u16,
            cmd: IDM_SEARCH_REPLACE as u16,
        },
        ACCEL {
            fVirt: ctrl_virt,
            key: b'G' as u16,
            cmd: IDM_SEARCH_GOTO_LINE as u16,
        },
        ACCEL {
            fVirt: virt_only,
            key: VK_F3,
            cmd: IDM_SEARCH_FIND_NEXT as u16,
        },
        ACCEL {
            fVirt: shift_virt,
            key: VK_F3,
            cmd: IDM_SEARCH_FIND_PREV as u16,
        },
    ];

    // SAFETY: accels is a valid, non-empty slice of ACCEL entries.
    let haccel = unsafe { CreateAcceleratorTableW(&accels) }.map_err(RivetError::from)?;
    Ok(haccel)
}

// ── Message loop ──────────────────────────────────────────────────────────────

fn message_loop(hwnd: HWND, haccel: HACCEL) -> Result<()> {
    let mut msg = MSG::default();
    loop {
        let ret = unsafe { GetMessageW(&mut msg, HWND::default(), 0, 0) };
        match ret.0 {
            -1 => return Err(last_error("GetMessageW")),
            0 => break,
            _ => unsafe {
                // Give the modeless Find/Replace dialog first crack at keyboard
                // messages (Tab, Enter, Escape, arrow keys, etc.).
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowState;
                let dlg = if !ptr.is_null() {
                    (*ptr).hwnd_find_dlg
                } else {
                    HWND::default()
                };
                if dlg != HWND::default() && IsDialogMessageW(dlg, &msg).as_bool() {
                    continue;
                }
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
    // Check for the registered "commdlg_FindReplace" message from the modeless
    // Find / Replace dialog before the standard match so it never falls through.
    if let Some(&find_msg) = FIND_MSG_ID.get() {
        if msg == find_msg {
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                handle_findreplace_msg(hwnd, lparam, &mut *ptr);
            }
            return LRESULT(0);
        }
    }

    match msg {
        // ── Startup ───────────────────────────────────────────────────────────
        WM_CREATE => {
            let hmodule = match GetModuleHandleW(None) {
                Ok(h) => h,
                Err(_) => return LRESULT(-1),
            };
            let hinstance = HINSTANCE(hmodule.0);

            match create_child_controls(hwnd, hinstance) {
                Ok(state) => {
                    let ptr = Box::into_raw(Box::new(state));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize);
                    post_create_init(hwnd, &mut *ptr);
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
            if !ptr.is_null() {
                // Collect the display names of every dirty tab.
                let dirty: Vec<String> = (*ptr)
                    .app
                    .tabs
                    .iter()
                    .filter(|doc| doc.dirty)
                    .map(|doc| doc.display_name())
                    .collect();

                if !dirty.is_empty() && !confirm_discard_all(hwnd, &dirty) {
                    return LRESULT(0);
                }

                // Save session while all Scintilla views are still alive.
                save_session(&*ptr);
            }
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }

        WM_DESTROY => {
            // Drop order: app → sci_views → sci_dll (FreeLibrary) → hwnd_*.
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                // Stop the auto-save timer before freeing state.
                // SAFETY: hwnd is valid; timer ID matches the one set in post_create_init.
                let _ = KillTimer(hwnd, AUTOSAVE_TIMER_ID);
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
                    if !ptr.is_null() {
                        handle_new_file(hwnd, &mut *ptr);
                    }
                    LRESULT(0)
                }
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
                IDM_FILE_CLOSE => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        handle_close_tab(hwnd, &mut *ptr, idx);
                    }
                    LRESULT(0)
                }
                IDM_FILE_EXIT => {
                    let _ = DestroyWindow(hwnd);
                    LRESULT(0)
                }

                // ── Edit commands ─────────────────────────────────────────────
                IDM_EDIT_UNDO => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        (*ptr).sci_views[idx].undo();
                    }
                    LRESULT(0)
                }
                IDM_EDIT_REDO => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        (*ptr).sci_views[idx].redo();
                    }
                    LRESULT(0)
                }
                IDM_EDIT_CUT => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        (*ptr).sci_views[idx].cut();
                    }
                    LRESULT(0)
                }
                IDM_EDIT_COPY => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        (*ptr).sci_views[idx].copy_to_clipboard();
                    }
                    LRESULT(0)
                }
                IDM_EDIT_PASTE => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        (*ptr).sci_views[idx].paste();
                    }
                    LRESULT(0)
                }
                IDM_EDIT_DELETE => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        (*ptr).sci_views[idx].delete_selection();
                    }
                    LRESULT(0)
                }
                IDM_EDIT_SELECT_ALL => {
                    if !ptr.is_null() {
                        let idx = (*ptr).app.active_idx;
                        (*ptr).sci_views[idx].select_all();
                    }
                    LRESULT(0)
                }

                // ── Format — EOL conversion ───────────────────────────────────
                IDM_FORMAT_EOL_CRLF => {
                    if !ptr.is_null() {
                        handle_eol_convert(hwnd, &mut *ptr, EolMode::Crlf);
                    }
                    LRESULT(0)
                }
                IDM_FORMAT_EOL_LF => {
                    if !ptr.is_null() {
                        handle_eol_convert(hwnd, &mut *ptr, EolMode::Lf);
                    }
                    LRESULT(0)
                }
                IDM_FORMAT_EOL_CR => {
                    if !ptr.is_null() {
                        handle_eol_convert(hwnd, &mut *ptr, EolMode::Cr);
                    }
                    LRESULT(0)
                }

                // ── View — Word Wrap ──────────────────────────────────────────
                IDM_VIEW_WORD_WRAP => {
                    if !ptr.is_null() {
                        handle_word_wrap_toggle(hwnd, &mut *ptr);
                    }
                    LRESULT(0)
                }

                // ── View — Dark Mode ──────────────────────────────────────────
                IDM_VIEW_DARK_MODE => {
                    if !ptr.is_null() {
                        handle_dark_mode_toggle(hwnd, &mut *ptr);
                    }
                    LRESULT(0)
                }

                // ── Search commands ───────────────────────────────────────────
                IDM_SEARCH_FIND => {
                    if !ptr.is_null() {
                        handle_find_open(hwnd, &mut *ptr);
                    }
                    LRESULT(0)
                }
                IDM_SEARCH_REPLACE => {
                    if !ptr.is_null() {
                        handle_replace_open(hwnd, &mut *ptr);
                    }
                    LRESULT(0)
                }
                IDM_SEARCH_FIND_NEXT => {
                    if !ptr.is_null() {
                        handle_find_next(hwnd, &mut *ptr, true);
                    }
                    LRESULT(0)
                }
                IDM_SEARCH_FIND_PREV => {
                    if !ptr.is_null() {
                        handle_find_next(hwnd, &mut *ptr, false);
                    }
                    LRESULT(0)
                }
                IDM_SEARCH_GOTO_LINE => {
                    if !ptr.is_null() {
                        let hmodule = GetModuleHandleW(None).unwrap_or_default();
                        let hinstance = HINSTANCE(hmodule.0);
                        handle_goto_line(hwnd, &mut *ptr, hinstance);
                    }
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
            if ptr.is_null() {
                return LRESULT(0);
            }

            match hdr.code {
                // ── Tab-control ───────────────────────────────────────────────
                TCN_SELCHANGE => {
                    // The tab control has already changed the selection; read it.
                    let sel = SendMessageW((*ptr).hwnd_tab, TCM_GETCURSEL, WPARAM(0), LPARAM(0));
                    if sel.0 < 0 {
                        return LRESULT(0);
                    } // shouldn't happen
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

                        // Reflect the new tab's word-wrap state in the View menu.
                        let wrap = (*ptr).app.active_doc().word_wrap;
                        update_wrap_checkmark(hwnd, wrap);

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

        // ── Periodic session checkpoint ───────────────────────────────────────
        WM_TIMER => {
            if wparam.0 == AUTOSAVE_TIMER_ID {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowState;
                if !ptr.is_null() {
                    save_session(&*ptr);
                }
            }
            LRESULT(0)
        }

        // ── DPI change ────────────────────────────────────────────────────────
        WM_DPICHANGED => {
            let new_dpi = (wparam.0 & 0xFFFF) as u32;
            let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut WindowState;
            if !ptr.is_null() {
                let state = &mut *ptr;
                state.dpi = new_dpi;
                // Windows provides the optimal new window bounds in LPARAM.
                // SAFETY: Windows guarantees LPARAM is a valid *const RECT for WM_DPICHANGED.
                let r = &*(lparam.0 as *const RECT);
                let _ = SetWindowPos(
                    hwnd,
                    HWND::default(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                update_statusbar_parts(state);
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
    let Some(path) = show_open_dialog(hwnd) else {
        return;
    };

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
            let _ = SendMessageW(state.hwnd_tab, TCM_SETCURSEL, WPARAM(dup_idx), LPARAM(0));
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
        Ok(b) => b,
        Err(e) => {
            show_error_dialog(&format!("Could not open file:\n{e}"));
            return;
        }
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
    apply_highlighting(
        &state.sci_views[idx],
        state.app.active_doc(),
        state.dark_mode,
    );
    state.sci_views[idx].set_eol_mode(eol);
    state.sci_views[idx].set_word_wrap(false); // always off on open; user toggles explicitly
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
        None => return,
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
    apply_highlighting(
        &state.sci_views[new_idx],
        state.app.active_doc(),
        state.dark_mode,
    );
    state.sci_views[new_idx].set_eol_mode(eol);
    state.sci_views[new_idx].set_word_wrap(false); // always off on open; user toggles explicitly
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
        None => return,
    };

    state.sci_views[state.app.active_idx].show(false);
    let new_idx = state.app.push_untitled();
    state.sci_views.push(sci);
    state.app.active_idx = new_idx;

    tab_insert(state.hwnd_tab, new_idx, "Untitled");
    let _ = SendMessageW(state.hwnd_tab, TCM_SETCURSEL, WPARAM(new_idx), LPARAM(0));

    // Apply Consolas font + current palette so all tabs are visually consistent.
    apply_highlighting(
        &state.sci_views[new_idx],
        state.app.active_doc(),
        state.dark_mode,
    );

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
        Ok(h) => h,
        Err(_) => return None,
    };
    let hinstance = HINSTANCE(hmodule.0);
    match ScintillaView::create(hwnd, hinstance, &state.sci_dll) {
        Ok(s) => Some(s),
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
            None => return,
        }
    } else {
        state.app.active_doc().path.clone().unwrap()
    };

    let idx = state.app.active_idx;
    let utf8 = state.sci_views[idx].get_text();
    match state.app.save(path, &utf8) {
        Ok(()) => {
            state.sci_views[idx].set_save_point();
            sync_tab_label(state, idx);
            update_window_title(hwnd, &state.app);
            // Refresh language in status bar (extension may have changed via Save As).
            update_status_bar(state);
        }
        Err(e) => show_error_dialog(&format!("Could not save file:\n{e}")),
    }
}

// ── EOL conversion ────────────────────────────────────────────────────────────

/// Handle Format > Convert to … : convert all existing EOL sequences and set
/// the new default EOL mode.  Scintilla fires `SCN_SAVEPOINTLEFT` automatically
/// after the conversion, so `doc.dirty` will be updated via the notification path.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_eol_convert(hwnd: HWND, state: &mut WindowState, eol: EolMode) {
    let idx = state.app.active_idx;
    // Convert all existing line endings and set the mode for new keystrokes.
    state.sci_views[idx].convert_eols(eol);
    state.sci_views[idx].set_eol_mode(eol);
    state.app.active_doc_mut().eol = eol;
    update_status_bar(state);
    let _ = hwnd; // hwnd available for future use (e.g. title update)
}

// ── Word wrap toggle ──────────────────────────────────────────────────────────

/// Handle View > Word Wrap: toggle word wrap for the active document.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_word_wrap_toggle(hwnd: HWND, state: &mut WindowState) {
    let wrap = !state.app.active_doc().word_wrap;
    state.app.active_doc_mut().word_wrap = wrap;
    let idx = state.app.active_idx;
    state.sci_views[idx].set_word_wrap(wrap);
    update_wrap_checkmark(hwnd, wrap);
}

/// Update the View > Word Wrap checkmark to reflect `wrap`.
///
/// Uses `MF_BYCOMMAND` so the correct item is found regardless of the menu
/// position of the View submenu (which shifted when Format was inserted).
///
/// # Safety
/// `hwnd` must be the valid main-window handle.
unsafe fn update_wrap_checkmark(hwnd: HWND, wrap: bool) {
    let menu = GetMenu(hwnd);
    // MF_BYCOMMAND | MF_{UN}CHECKED gives MENU_ITEM_FLAGS; CheckMenuItem wants u32.
    let flag = (MF_BYCOMMAND | if wrap { MF_CHECKED } else { MF_UNCHECKED }).0;
    // SAFETY: menu is the main window's menu bar (valid while the window exists).
    // CheckMenuItem with MF_BYCOMMAND searches all submenus.
    let _ = CheckMenuItem(menu, IDM_VIEW_WORD_WRAP as u32, flag);
}

// ── DPI + status bar helpers ─────────────────────────────────────────────────

/// Initialise DPI tracking and apply initial highlighting to the first tab.
///
/// Called from WM_CREATE after the `WindowState` is stored in GWLP_USERDATA.
///
/// # Safety
/// `hwnd` must be the valid main-window handle; `state` must be live.
unsafe fn post_create_init(hwnd: HWND, state: &mut WindowState) {
    state.dpi = crate::platform::win32::dpi::get_for_window(hwnd);
    if state.dpi != crate::platform::win32::dpi::BASE_DPI {
        update_statusbar_parts(state);
    }
    // Apply Consolas font + initial palette to the first untitled tab.
    apply_highlighting(&state.sci_views[0], state.app.active_doc(), state.dark_mode);
    // Start the periodic session checkpoint timer.
    // SAFETY: hwnd is valid; no callback (None) — the timer fires as WM_TIMER.
    let _ = SetTimer(hwnd, AUTOSAVE_TIMER_ID, AUTOSAVE_INTERVAL_MS, None);
}

/// Recompute and apply DPI-scaled status-bar part widths.
fn update_statusbar_parts(state: &WindowState) {
    use crate::platform::win32::dpi;
    let enc = dpi::scale(SB_PART_ENCODING_W_BASE, state.dpi);
    let eol = dpi::scale(SB_PART_EOL_W_BASE, state.dpi);
    let lang = dpi::scale(SB_PART_LANG_W_BASE, state.dpi);
    let parts: [i32; 4] = [enc, enc + eol, enc + eol + lang, -1];
    // SAFETY: hwnd_status is a valid status-bar HWND for the lifetime of WindowState.
    unsafe {
        let _ = SendMessageW(
            state.hwnd_status,
            SB_SETPARTS,
            WPARAM(parts.len()),
            LPARAM(parts.as_ptr() as isize),
        );
    }
}

// ── Dark mode helpers ─────────────────────────────────────────────────────────

/// Toggle dark mode: flip flag, update chrome + checkmark, re-theme all views.
///
/// # Safety
/// `hwnd` must be the valid main-window handle; `state` must be live.
unsafe fn handle_dark_mode_toggle(hwnd: HWND, state: &mut WindowState) {
    state.dark_mode = !state.dark_mode;
    apply_title_bar_dark(hwnd, state.dark_mode);
    update_dark_mode_checkmark(hwnd, state.dark_mode);
    reapply_all_themes(state);
}

/// Set or clear the View > Dark Mode checkmark.
///
/// # Safety
/// `hwnd` must be the valid main-window handle.
unsafe fn update_dark_mode_checkmark(hwnd: HWND, dark: bool) {
    let flag = (MF_BYCOMMAND | if dark { MF_CHECKED } else { MF_UNCHECKED }).0;
    let _ = CheckMenuItem(GetMenu(hwnd), IDM_VIEW_DARK_MODE as u32, flag);
}

/// Apply or remove dark DWM window chrome (title bar).
///
/// Silently ignored on unsupported Windows versions.
fn apply_title_bar_dark(hwnd: HWND, dark: bool) {
    use windows::Win32::Graphics::Dwm::{DwmSetWindowAttribute, DWMWINDOWATTRIBUTE};
    let value: u32 = dark as u32;
    // SAFETY: hwnd is a valid window handle; pvAttribute points to a u32 whose
    // size matches cbAttribute.
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWINDOWATTRIBUTE(DWMWA_DARK_MODE),
            &value as *const u32 as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Re-apply highlighting (with the current `dark_mode` flag) to every open tab.
fn reapply_all_themes(state: &mut WindowState) {
    for i in 0..state.app.tabs.len() {
        apply_highlighting(&state.sci_views[i], &state.app.tabs[i], state.dark_mode);
    }
}

// ── Find / Replace helpers ────────────────────────────────────────────────────

/// Open (or focus) the modeless Find dialog.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_find_open(hwnd: HWND, state: &mut WindowState) {
    if state.hwnd_find_dlg != HWND::default() {
        // Dialog already open — bring it to the front.
        let _ = SetForegroundWindow(state.hwnd_find_dlg);
        return;
    }
    state.findreplace.hwndOwner = hwnd;
    // Clear the replace-only flag so FindTextW shows the Find dialog.
    state.findreplace.Flags =
        FINDREPLACE_FLAGS((state.findreplace.Flags.0 & !(FR_REPLACE | FR_REPLACEALL)) | FR_DOWN);
    // SAFETY: findreplace is stable in heap memory; hwndOwner is valid.
    // FindTextW returns HWND directly (null = failure), same as CreateWindowExW.
    state.hwnd_find_dlg = FindTextW(&mut state.findreplace);
}

/// Open (or focus) the modeless Replace dialog.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_replace_open(hwnd: HWND, state: &mut WindowState) {
    if state.hwnd_find_dlg != HWND::default() {
        let _ = SetForegroundWindow(state.hwnd_find_dlg);
        return;
    }
    state.findreplace.hwndOwner = hwnd;
    state.findreplace.Flags = FINDREPLACE_FLAGS(state.findreplace.Flags.0 | FR_DOWN);
    // SAFETY: findreplace is stable in heap memory; hwndOwner is valid.
    state.hwnd_find_dlg = ReplaceTextW(&mut state.findreplace);
}

/// Handle the registered "commdlg_FindReplace" message sent by FindTextW /
/// ReplaceTextW whenever the user clicks Find Next, Replace, Replace All, or
/// closes the dialog.
///
/// # Safety
/// `lparam` is a valid `*const FINDREPLACEW` provided by the OS.
unsafe fn handle_findreplace_msg(hwnd: HWND, lparam: LPARAM, state: &mut WindowState) {
    // SAFETY: the OS guarantees lparam is a *const FINDREPLACEW pointing to
    // the same struct we passed to FindTextW / ReplaceTextW.
    let fr = &*(lparam.0 as *const FINDREPLACEW);
    let flags = fr.Flags.0;

    if flags & FR_DIALOGTERM != 0 {
        // Dialog is closing — clear the stored HWND.
        state.hwnd_find_dlg = HWND::default();
        return;
    }

    let find_bytes = pwstr_to_utf8(fr.lpstrFindWhat);
    if find_bytes.is_empty() {
        return;
    }

    let sci_flags = (if flags & FR_MATCHCASE != 0 {
        SCFIND_MATCHCASE
    } else {
        0
    }) | (if flags & FR_WHOLEWORD != 0 {
        SCFIND_WHOLEWORD
    } else {
        0
    });
    let forward = flags & FR_DOWN != 0;

    let idx = state.app.active_idx;
    let sci = &state.sci_views[idx];

    if flags & FR_FINDNEXT != 0 {
        if !sci.find_next(&find_bytes, sci_flags, forward) {
            let _ = MessageBeep(MESSAGEBOX_STYLE(0xFFFF_FFFF));
        }
    } else if flags & FR_REPLACE != 0 {
        let repl_bytes = pwstr_to_utf8(fr.lpstrReplaceWith);
        handle_replace_once(sci, &find_bytes, &repl_bytes, sci_flags, forward);
    } else if flags & FR_REPLACEALL != 0 {
        let repl_bytes = pwstr_to_utf8(fr.lpstrReplaceWith);
        let n = sci.replace_all(&find_bytes, &repl_bytes, sci_flags);
        let msg = format!("{n} replacement{} made.", if n == 1 { "" } else { "s" });
        let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        let _ = MessageBoxW(hwnd, PCWSTR(wide.as_ptr()), w!("Rivet"), MB_OK);
    }
}

/// Replace the current selection (if it matches `find`) then move to the next
/// occurrence.
///
/// # Safety
/// `sci` must be a valid `ScintillaView` whose HWND is alive.
unsafe fn handle_replace_once(
    sci: &ScintillaView,
    find: &[u8],
    repl: &[u8],
    flags: u32,
    forward: bool,
) {
    let sel_start = sci.selection_start();
    let sel_end = sci.selection_end();

    // If the current selection exactly matches the search term, replace it.
    if sel_end > sel_start {
        sci.set_target(sel_start, sel_end);
        if sci.search_in_target(find, flags).is_some() {
            sci.replace_target(repl);
        }
    }

    // Advance to the next match.
    if !sci.find_next(find, flags, forward) {
        let _ = MessageBeep(MESSAGEBOX_STYLE(0xFFFF_FFFF));
    }
}

/// Handle F3 / Shift+F3: repeat the last search from the Find dialog.
///
/// If no previous search text exists in the buffer the Find dialog is opened.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_find_next(hwnd: HWND, state: &mut WindowState, forward: bool) {
    // If the find buffer is empty (no previous search), open the Find dialog.
    if state.find_buf[0] == 0 {
        handle_find_open(hwnd, state);
        return;
    }

    // Derive Scintilla flags from the last dialog flag state.
    let fr_flags = state.findreplace.Flags.0;
    let sci_flags = (if fr_flags & FR_MATCHCASE != 0 {
        SCFIND_MATCHCASE
    } else {
        0
    }) | (if fr_flags & FR_WHOLEWORD != 0 {
        SCFIND_WHOLEWORD
    } else {
        0
    });

    // Decode the UTF-16 find buffer to UTF-8.
    let len = state.find_buf.iter().position(|&c| c == 0).unwrap_or(0);
    let s = String::from_utf16_lossy(&state.find_buf[..len]);
    let find_bytes = s.into_bytes();

    let idx = state.app.active_idx;
    if !state.sci_views[idx].find_next(&find_bytes, sci_flags, forward) {
        let _ = MessageBeep(MESSAGEBOX_STYLE(0xFFFF_FFFF));
    }
}

/// Handle Search > Go to Line: show a modal dialog and jump the caret.
///
/// # Safety
/// Called only from WM_COMMAND on the UI thread with a valid `state`.
unsafe fn handle_goto_line(hwnd: HWND, state: &mut WindowState, hinstance: HINSTANCE) {
    let idx = state.app.active_idx;
    let total = state.sci_views[idx].line_count();
    let (current, _) = state.sci_views[idx].caret_line_col(); // 1-based

    if let Some(target) = show_goto_line_dialog(hwnd, hinstance, current, total) {
        if target >= 1 && target <= total {
            let pos = state.sci_views[idx].position_from_line(target - 1); // 0-based
            state.sci_views[idx].set_caret_pos(pos);
            state.sci_views[idx].scroll_caret();
        }
    }
}

// ── Go To Line dialog ─────────────────────────────────────────────────────────

/// Data passed to `goto_dlg_proc` via the `lParam` of `WM_INITDIALOG`.
struct GotoLineParams {
    current: usize, // 1-based current line (pre-filled in the edit)
    total: usize,   // total lines (upper bound for validation)
}

/// Show a modal "Go to Line" dialog.
///
/// Returns `Some(n)` (1-based) if the user confirmed a valid line number,
/// `None` if they cancelled or entered an invalid value.
///
/// # Safety
/// `hwnd_parent` and `hinstance` must be valid Win32 handles.
unsafe fn show_goto_line_dialog(
    hwnd_parent: HWND,
    hinstance: HINSTANCE,
    current_line: usize,
    total_lines: usize,
) -> Option<usize> {
    let template = build_goto_line_template(total_lines);
    let params = GotoLineParams {
        current: current_line,
        total: total_lines,
    };

    // SAFETY: template contains a correctly structured DLGTEMPLATE byte blob;
    // goto_dlg_proc is a valid DLGPROC; params lives for the duration of the
    // modal dialog (DialogBoxIndirectParamW blocks until EndDialog is called).
    let result = DialogBoxIndirectParamW(
        hinstance,
        template.as_ptr() as *const DLGTEMPLATE,
        hwnd_parent,
        Some(goto_dlg_proc),
        LPARAM(&params as *const GotoLineParams as isize),
    );

    if result > 0 {
        Some(result as usize)
    } else {
        None
    }
}

/// Dialog procedure for the "Go to Line" modal dialog.
///
/// # Safety
/// Called by Windows with valid arguments for the lifetime of the dialog.
unsafe extern "system" fn goto_dlg_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    const EDIT_ID: i32 = 100;
    const EM_SETSEL: u32 = 0x00B1;

    match msg {
        WM_INITDIALOG => {
            // Store the params pointer so WM_COMMAND can read `total`.
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, lparam.0);
            let params = &*(lparam.0 as *const GotoLineParams);

            // Pre-fill the edit with the current line number.
            let text: Vec<u16> = format!("{}", params.current)
                .encode_utf16()
                .chain(std::iter::once(0))
                .collect();
            let _ = SetDlgItemTextW(hwnd, EDIT_ID, PCWSTR(text.as_ptr()));

            // Select all text in the edit so the user can type immediately.
            if let Ok(edit) = GetDlgItem(hwnd, EDIT_ID) {
                let _ = SendMessageW(edit, EM_SETSEL, WPARAM(0), LPARAM(-1isize));
            }

            1 // TRUE: let Windows set focus to the first focusable control
        }

        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as u16;
            match id {
                1 => {
                    // IDOK — validate the input and close.
                    let params_ptr =
                        GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const GotoLineParams;
                    let total = if !params_ptr.is_null() {
                        (*params_ptr).total
                    } else {
                        usize::MAX
                    };

                    let mut buf = [0u16; 32];
                    let len = GetDlgItemTextW(hwnd, EDIT_ID, &mut buf);
                    let s = String::from_utf16_lossy(&buf[..len as usize]);
                    match s.trim().parse::<usize>() {
                        Ok(n) if n >= 1 && n <= total => {
                            let _ = EndDialog(hwnd, n as isize);
                        }
                        _ => {
                            // Invalid input — beep and keep the dialog open.
                            let _ = MessageBeep(MESSAGEBOX_STYLE(0xFFFF_FFFF));
                        }
                    }
                    0
                }
                2 => {
                    // IDCANCEL — close without navigating.
                    let _ = EndDialog(hwnd, 0);
                    0
                }
                _ => 0,
            }
        }

        _ => 0,
    }
}

/// Build a minimal in-memory `DLGTEMPLATE` for the "Go to Line" dialog.
///
/// Layout (185 × 55 dialog units, centred by DS_CENTER):
///   Label  "Go to line (1–N):"  at (7, 7)  170×9 DU
///   Edit   (ID=100)             at (7, 18)  170×14 DU
///   OK     (IDOK=1)             at (73, 36) 50×14 DU
///   Cancel (IDCANCEL=2)         at (128, 36) 50×14 DU
fn build_goto_line_template(total_lines: usize) -> Vec<u8> {
    // ── Local bit constants (u32 to avoid conflict with WINDOW_STYLE newtypes) ──
    const WS_POPUP_V: u32 = 0x8000_0000;
    const WS_CAPTION_V: u32 = 0x00C0_0000; // WS_BORDER | WS_DLGFRAME
    const WS_SYSMENU_V: u32 = 0x0008_0000;
    const DS_MODALFRAME: u32 = 0x0080;
    const DS_CENTER: u32 = 0x0800;
    const WS_CHILD_V: u32 = 0x4000_0000;
    const WS_VISIBLE_V: u32 = 0x1000_0000;
    const WS_BORDER_V: u32 = 0x0080_0000;
    const WS_TABSTOP_V: u32 = 0x0001_0000;
    const ES_AUTOHSCROLL: u32 = 0x0080;
    const BS_DEFPB: u32 = 0x0001; // BS_DEFPUSHBUTTON
                                  // Predefined class atoms for controls in a dialog template.
    const ATOM_BUTTON: u16 = 0x0080;
    const ATOM_EDIT: u16 = 0x0081;
    const ATOM_STATIC: u16 = 0x0082;

    let dlg_style: u32 = WS_POPUP_V | WS_CAPTION_V | WS_SYSMENU_V | DS_MODALFRAME | DS_CENTER;

    let label = format!("Go to line (1\u{2013}{total_lines}):");

    let mut v: Vec<u8> = Vec::with_capacity(512);

    // ── DLGTEMPLATE header ────────────────────────────────────────────────────
    push_u32(&mut v, dlg_style);
    push_u32(&mut v, 0); // dwExtendedStyle
    push_u16(&mut v, 4); // cdit — number of controls
    push_u16(&mut v, 0); // x (DS_CENTER ignores these)
    push_u16(&mut v, 0); // y
    push_u16(&mut v, 185); // cx (dialog units)
    push_u16(&mut v, 55); // cy
    push_u16(&mut v, 0); // menu: none
    push_u16(&mut v, 0); // window class: default dialog
    push_wstr(&mut v, "Go to Line"); // title

    // ── Control 1: Static label ───────────────────────────────────────────────
    align4(&mut v);
    push_u32(&mut v, WS_CHILD_V | WS_VISIBLE_V); // SS_LEFT = 0
    push_u32(&mut v, 0);
    push_u16(&mut v, 7);
    push_u16(&mut v, 7);
    push_u16(&mut v, 170);
    push_u16(&mut v, 9);
    push_u16(&mut v, 0xFFFF); // id (unused for statics)
    push_u16(&mut v, 0xFFFF);
    push_u16(&mut v, ATOM_STATIC);
    push_wstr(&mut v, &label);
    push_u16(&mut v, 0); // cbWndExtra

    // ── Control 2: Edit (ID=100) ──────────────────────────────────────────────
    align4(&mut v);
    push_u32(
        &mut v,
        WS_CHILD_V | WS_VISIBLE_V | WS_BORDER_V | WS_TABSTOP_V | ES_AUTOHSCROLL,
    );
    push_u32(&mut v, 0);
    push_u16(&mut v, 7);
    push_u16(&mut v, 18);
    push_u16(&mut v, 170);
    push_u16(&mut v, 14);
    push_u16(&mut v, 100); // id=100
    push_u16(&mut v, 0xFFFF);
    push_u16(&mut v, ATOM_EDIT);
    push_wstr(&mut v, "");
    push_u16(&mut v, 0);

    // ── Control 3: OK button (IDOK=1) ─────────────────────────────────────────
    align4(&mut v);
    push_u32(&mut v, WS_CHILD_V | WS_VISIBLE_V | WS_TABSTOP_V | BS_DEFPB);
    push_u32(&mut v, 0);
    push_u16(&mut v, 73);
    push_u16(&mut v, 36);
    push_u16(&mut v, 50);
    push_u16(&mut v, 14);
    push_u16(&mut v, 1); // IDOK
    push_u16(&mut v, 0xFFFF);
    push_u16(&mut v, ATOM_BUTTON);
    push_wstr(&mut v, "OK");
    push_u16(&mut v, 0);

    // ── Control 4: Cancel button (IDCANCEL=2) ─────────────────────────────────
    align4(&mut v);
    push_u32(&mut v, WS_CHILD_V | WS_VISIBLE_V | WS_TABSTOP_V);
    push_u32(&mut v, 0);
    push_u16(&mut v, 128);
    push_u16(&mut v, 36);
    push_u16(&mut v, 50);
    push_u16(&mut v, 14);
    push_u16(&mut v, 2); // IDCANCEL
    push_u16(&mut v, 0xFFFF);
    push_u16(&mut v, ATOM_BUTTON);
    push_wstr(&mut v, "Cancel");
    push_u16(&mut v, 0);

    v
}

// ── DLGTEMPLATE builder helpers ───────────────────────────────────────────────

#[inline]
fn push_u16(v: &mut Vec<u8>, n: u16) {
    v.extend_from_slice(&n.to_le_bytes());
}

#[inline]
fn push_u32(v: &mut Vec<u8>, n: u32) {
    v.extend_from_slice(&n.to_le_bytes());
}

/// Append a null-terminated UTF-16 string.
fn push_wstr(v: &mut Vec<u8>, s: &str) {
    for cu in s.encode_utf16() {
        push_u16(v, cu);
    }
    push_u16(v, 0); // null terminator
}

/// Pad to the next 4-byte boundary (required between DLGITEMTEMPLATE entries).
fn align4(v: &mut Vec<u8>) {
    while v.len() % 4 != 0 {
        v.push(0);
    }
}

// ── PWSTR → UTF-8 helper ──────────────────────────────────────────────────────

/// Convert a null-terminated Win32 wide string to a UTF-8 `Vec<u8>`.
///
/// Returns an empty Vec if the pointer is null or the string is invalid UTF-16.
///
/// # Safety
/// `pwstr` must be a valid null-terminated UTF-16 string for the duration of
/// this call (guaranteed by the FINDREPLACEW dialog contract).
unsafe fn pwstr_to_utf8(pwstr: PWSTR) -> Vec<u8> {
    if pwstr.is_null() {
        return Vec::new();
    }
    // SAFETY: caller guarantees pwstr is a valid null-terminated UTF-16 string.
    pwstr
        .to_string()
        .map(|s| s.into_bytes())
        .unwrap_or_default()
}

// ── Status bar / title ────────────────────────────────────────────────────────

// Refresh all three status-bar parts from the current `WindowState`.
// Parts:  0 = encoding  |  1 = EOL mode  |  2 = Ln / Col
// Safety: `state.hwnd_status` and the active sci_view must be valid.
// ── Syntax highlighting ────────────────────────────────────────────────────────

/// Apply the language lexer and colour theme to `sci` based on `doc`.
///
/// Skipped for large files (`doc.large_file == true`) — they stay with
/// `SCLEX_NULL` (plain text) which is already set by `set_large_file_mode`.
fn apply_highlighting(sci: &ScintillaView, doc: &crate::app::DocumentState, dark: bool) {
    if doc.large_file {
        return;
    }
    let lang = match &doc.path {
        Some(p) => crate::languages::language_from_path(p),
        None => crate::languages::Language::PlainText,
    };
    sci.set_lexer(lang.lexer_id());
    for (set_idx, words) in crate::languages::keywords(lang) {
        sci.set_keywords(*set_idx, words);
    }
    crate::theme::apply_theme(sci, lang, dark);
}

unsafe fn update_status_bar(state: &WindowState) {
    let idx = state.app.active_idx;
    let (line, col) = state.sci_views[idx].caret_line_col();
    let (enc, eol, large_file, path) = {
        let doc = state.app.active_doc();
        (
            doc.encoding.as_str().to_owned(),
            doc.eol.as_str().to_owned(),
            doc.large_file,
            doc.path.clone(),
        )
    };
    let lang = match &path {
        Some(p) => crate::languages::language_from_path(p),
        None => crate::languages::Language::PlainText,
    };
    let lang_text = if large_file {
        format!("{} [Large]", lang.display_name())
    } else {
        lang.display_name().to_owned()
    };
    // Parts: 0=encoding, 1=EOL, 2=Ln/Col, 3=language
    let texts: [String; 4] = [enc, eol, format!("Ln {line}, Col {col}"), lang_text];
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

// ── Close tab ─────────────────────────────────────────────────────────────────

/// Close the tab at `idx`, prompting about unsaved changes if needed.
///
/// If `idx` is the last remaining tab the editor content is cleared and the
/// tab is reset to an untitled document instead of being removed (so there is
/// always at least one tab).
///
/// # Safety
/// Called only from WM_COMMAND / accelerator on the UI thread.
unsafe fn handle_close_tab(hwnd: HWND, state: &mut WindowState, idx: usize) {
    // ── Dirty check ───────────────────────────────────────────────────────────
    if state.app.tabs[idx].dirty {
        let name = state.app.tabs[idx].display_name();
        let msg = format!("\"{name}\" has unsaved changes.\n\nSave before closing?");
        let wide: Vec<u16> = msg.encode_utf16().chain(std::iter::once(0)).collect();
        // SAFETY: wide is valid null-terminated UTF-16 that outlives the call.
        let result = MessageBoxW(
            hwnd,
            PCWSTR(wide.as_ptr()),
            w!("Rivet"),
            MB_YESNOCANCEL | MB_ICONWARNING,
        );
        match result {
            r if r == IDYES => {
                // Try to save; if it fails or the user cancels the dialog, abort.
                if !save_tab_for_close(hwnd, state, idx) {
                    return;
                }
            }
            r if r == IDNO => { /* discard — fall through to close */ }
            _ => return, // Cancel
        }
    }

    // ── Last tab: reset to untitled instead of removing ───────────────────────
    if state.app.tab_count() == 1 {
        let doc = &mut state.app.tabs[0];
        doc.path = None;
        doc.dirty = false;
        doc.large_file = false;
        doc.encoding = crate::app::Encoding::Utf8;
        doc.eol = crate::app::EolMode::Crlf;
        doc.word_wrap = false;
        state.sci_views[0].set_eol_mode(crate::app::EolMode::Crlf);
        state.sci_views[0].set_word_wrap(false);
        state.sci_views[0].set_text(b"");
        state.sci_views[0].set_save_point();
        update_wrap_checkmark(hwnd, false);
        sync_tab_label(state, 0);
        update_window_title(hwnd, &state.app);
        update_status_bar(state);
        return;
    }

    // ── Remove the tab ────────────────────────────────────────────────────────
    let was_active = idx == state.app.active_idx;

    // Explicitly destroy the child HWND (parent window is still alive).
    state.sci_views[idx].destroy();
    state.sci_views.remove(idx);

    // Remove the tab strip entry.
    let _ = SendMessageW(state.hwnd_tab, TCM_DELETEITEM, WPARAM(idx), LPARAM(0));

    // Update App state; remove_tab returns the new active_idx.
    let new_active = state.app.remove_tab(idx);

    // Sync the tab strip selection.
    let _ = SendMessageW(state.hwnd_tab, TCM_SETCURSEL, WPARAM(new_active), LPARAM(0));

    // If we closed the active tab, make the new active view visible.
    if was_active {
        state.sci_views[new_active].show(true);
    }

    // Resize the (possibly newly visible) active view.
    let mut rc = RECT::default();
    let _ = GetClientRect(hwnd, &mut rc);
    layout_children(state, rc.right, rc.bottom);

    update_window_title(hwnd, &state.app);
    update_status_bar(state);
}

/// Save the tab at `idx` in preparation for closing it.
///
/// If the tab has no path a Save-As dialog is shown.  Returns `true` if the
/// save succeeded and the close should proceed; `false` if the save failed or
/// the user cancelled the dialog.
///
/// Uses `App::save` by temporarily pointing `active_idx` at `idx`.  The caller
/// closes the tab immediately on success, so the temporary change is benign.
///
/// # Safety
/// Called only from `handle_close_tab` on the UI thread with a valid `state`.
unsafe fn save_tab_for_close(hwnd: HWND, state: &mut WindowState, idx: usize) -> bool {
    let path = if let Some(p) = state.app.tabs[idx].path.clone() {
        p
    } else {
        match show_save_dialog(hwnd, "") {
            Some(p) => p,
            None => return false, // user cancelled the dialog
        }
    };

    let utf8 = state.sci_views[idx].get_text();

    // Redirect App::save to the correct document by temporarily adjusting
    // active_idx; restore it on failure so the visible state is consistent.
    let prev_active = state.app.active_idx;
    state.app.active_idx = idx;

    match state.app.save(path, &utf8) {
        Ok(()) => {
            state.sci_views[idx].set_save_point();
            sync_tab_label(state, idx);
            // Leave active_idx at idx — handle_close_tab removes it next.
            true
        }
        Err(e) => {
            state.app.active_idx = prev_active;
            show_error_dialog(&format!("Could not save file:\n{e}"));
            false
        }
    }
}

/// Combined exit guard: show a single dialog listing every dirty tab.
///
/// Returns `true` if the user chose to discard all changes and exit.
///
/// # Safety
/// `hwnd` must be a valid window handle.
unsafe fn confirm_discard_all(hwnd: HWND, names: &[String]) -> bool {
    let mut text = String::from("The following files have unsaved changes:\n");
    for name in names {
        text.push_str(&format!("  \u{2022} {name}\n"));
    }
    text.push_str("\nDiscard all and exit?");

    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // MB_YESNO: "Yes" = discard and exit, "No" = stay open.
    let result = MessageBoxW(
        hwnd,
        PCWSTR(wide.as_ptr()),
        w!("Rivet"),
        MB_YESNO | MB_ICONWARNING,
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

// ── Session ───────────────────────────────────────────────────────────────────

/// Serialize the current session to `%APPDATA%\Rivet\session.json`.
///
/// Must be called while all Scintilla child windows are still alive (i.e.
/// from `WM_CLOSE`, before `DestroyWindow`).  Errors are silently discarded.
fn save_session(state: &WindowState) {
    let entries: Vec<crate::session::TabEntry> = state
        .app
        .tabs
        .iter()
        .enumerate()
        .map(|(i, doc)| crate::session::TabEntry {
            path: doc.path.as_ref().map(|p| p.to_string_lossy().into_owned()),
            caret_pos: state.sci_views[i].caret_pos(),
            scroll_line: state.sci_views[i].first_visible_line(),
            encoding: doc.encoding.as_str().to_owned(),
            eol: doc.eol.as_str().to_owned(),
        })
        .collect();

    let _ = crate::session::save(&entries, state.app.active_idx, state.dark_mode);
}

/// Re-open the tabs recorded in the session file.
///
/// Called once from `run()` after the main window is visible.  Entries without
/// a path (untitled buffers) and entries whose file no longer exists on disk
/// are silently skipped.  On any error the function returns early, leaving the
/// initial untitled tab intact.
///
/// # Safety
/// `hwnd` must be the valid main-window handle; `state` must point to a live
/// `WindowState`.
unsafe fn restore_session(hwnd: HWND, state: &mut WindowState) {
    let Some(sf) = crate::session::load() else {
        return;
    };

    // Restore dark mode BEFORE loading files so each apply_highlighting call
    // uses the correct palette.
    if sf.dark_mode {
        state.dark_mode = true;
        apply_title_bar_dark(hwnd, true);
        update_dark_mode_checkmark(hwnd, true);
    }

    let mut opened_any = false;

    for entry in &sf.tabs {
        let Some(path_str) = &entry.path else {
            continue;
        };
        let path = std::path::PathBuf::from(path_str);
        if !path.exists() {
            continue;
        }

        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };

        if !opened_any {
            // Reuse the initial untitled tab for the first restored file.
            load_file_into_active_tab(hwnd, state, path, &bytes);
        } else {
            open_file_in_new_tab(hwnd, state, path, &bytes);
        }

        // Restore caret and scroll.  SCI_GOTOPOS clamps to document length
        // if the position is beyond the end of file, so no bounds check needed.
        let idx = state.app.active_idx;
        state.sci_views[idx].set_caret_pos(entry.caret_pos);
        state.sci_views[idx].set_first_visible_line(entry.scroll_line);

        opened_any = true;
    }

    if !opened_any {
        return;
    }

    // Restore the active tab (clamped to the number of tabs we actually opened).
    let target = sf.active_tab.min(state.app.tab_count() - 1);
    if target != state.app.active_idx {
        state.sci_views[state.app.active_idx].show(false);
        state.app.active_idx = target;
        state.sci_views[target].show(true);
        let _ = SendMessageW(state.hwnd_tab, TCM_SETCURSEL, WPARAM(target), LPARAM(0));
        let eol = state.sci_views[target].eol_mode();
        state.app.active_doc_mut().eol = eol;

        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        layout_children(state, rc.right, rc.bottom);
    }

    update_window_title(hwnd, &state.app);
    update_status_bar(state);
}

// ── Error helpers ─────────────────────────────────────────────────────────────

fn last_error(function: &'static str) -> RivetError {
    // SAFETY: GetLastError reads thread-local state set by the failing call.
    let code = unsafe { GetLastError() };
    RivetError::Win32 {
        function,
        code: code.0,
    }
}
