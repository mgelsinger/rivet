// ── Tab bar state ─────────────────────────────────────────────────────────────
//
// Pure Rust state that mirrors the Win32 SysTabControl32 content.
// No Win32 calls here; all control messages are sent from `platform::win32::window`.

use crate::app::DocumentState;

/// Compute the display label for a tab from its document state.
///
/// Format:
/// - Untitled, clean  → `"Untitled"`
/// - Untitled, dirty  → `"*Untitled"`
/// - Named, clean     → `"filename.txt"`
/// - Named, dirty     → `"*filename.txt"`
pub(crate) fn tab_label(doc: &DocumentState) -> String {
    let name = doc
        .path
        .as_deref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Untitled".to_owned());
    if doc.dirty {
        format!("*{name}")
    } else {
        name
    }
}
