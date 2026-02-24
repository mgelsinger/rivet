// ── Safety policy ────────────────────────────────────────────────────────────
// Unsafe code is forbidden everywhere except:
//   • `platform::win32`   – Win32 / WinAPI FFI
//   • `editor::scintilla` – Scintilla child-window hosting
// Each unsafe block in those modules MUST carry a `// SAFETY:` comment.
#![deny(unsafe_code)]

// Release builds run as a GUI application (no console window).
// Debug builds keep the console so that eprintln! timing output is visible.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod editor;
mod error;
mod platform;

fn main() {
    if let Err(e) = platform::win32::window::run() {
        // Startup failed before or during the message loop.
        // Show a modal error dialog — the only safe output path in a GUI app.
        platform::win32::window::show_error_dialog(&e.to_string());
        std::process::exit(1);
    }
}
