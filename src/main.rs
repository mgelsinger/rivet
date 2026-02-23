// ── Safety policy ────────────────────────────────────────────────────────────
// Unsafe code is forbidden everywhere except:
//   • `platform::win32`   – Win32 / WinAPI FFI
//   • `editor::scintilla` – Scintilla child-window hosting
// Each unsafe block in those modules MUST carry a `// SAFETY:` comment.
#![deny(unsafe_code)]

// Phase 1: module skeletons wired in; implementations arrive in Phase 2.
mod editor;
mod platform;

fn main() {
    // Phase 0 stub – replaced in Phase 2 with the real WinMain entry point.
    println!("Rivet – not yet implemented");
}
