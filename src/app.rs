// ── Application lifecycle & top-level state ────────────────────────────────────
//
// Pure Rust — no Win32 imports.  `App` holds the document-state vector and
// the active-tab index.  The parallel `Vec<ScintillaView>` lives in
// `platform::win32::WindowState` so that this module stays testable without
// a Win32 environment.

use std::path::PathBuf;

use crate::editor::LARGE_FILE_THRESHOLD_BYTES;

// ── Encoding ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    Utf8,
    Utf16Le,
    Utf16Be,
    Ansi,
}

impl Encoding {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Ansi => "ANSI",
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "UTF-8" => Some(Self::Utf8),
            "UTF-16 LE" => Some(Self::Utf16Le),
            "UTF-16 BE" => Some(Self::Utf16Be),
            "ANSI" => Some(Self::Ansi),
            _ => None,
        }
    }
}

// ── EOL mode ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EolMode {
    Crlf,
    Lf,
    Cr,
}

impl EolMode {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Crlf => "CRLF",
            Self::Lf => "LF",
            Self::Cr => "CR",
        }
    }

    #[allow(dead_code)]
    pub(crate) fn from_str(s: &str) -> Option<Self> {
        match s {
            "CRLF" => Some(Self::Crlf),
            "LF" => Some(Self::Lf),
            "CR" => Some(Self::Cr),
            _ => None,
        }
    }
}

// ── DocumentState ─────────────────────────────────────────────────────────────

/// Per-document state.
///
/// Phase 4 keeps one `DocumentState` per tab in `App::tabs`.
/// The matching `ScintillaView` lives in `WindowState::sci_views` at the same index.
#[derive(Debug)]
pub(crate) struct DocumentState {
    pub(crate) path: Option<PathBuf>,
    pub(crate) encoding: Encoding,
    pub(crate) eol: EolMode,
    pub(crate) dirty: bool,
    pub(crate) large_file: bool,
    pub(crate) word_wrap: bool,
}

impl DocumentState {
    pub(crate) fn new_untitled() -> Self {
        Self {
            path: None,
            encoding: Encoding::Utf8,
            eol: EolMode::Crlf,
            dirty: false,
            large_file: false,
            word_wrap: false,
        }
    }

    /// Bare filename for display, or `"Untitled"`.
    pub(crate) fn display_name(&self) -> String {
        self.path
            .as_deref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_owned())
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

/// Top-level application state.
///
/// Always holds at least one tab (`tabs` is never empty).
/// The parallel `Vec<ScintillaView>` in `WindowState` must stay the same length.
pub(crate) struct App {
    /// Document state for every open tab.
    pub(crate) tabs: Vec<DocumentState>,
    /// Index of the currently visible tab.
    pub(crate) active_idx: usize,
}

impl App {
    /// Create an `App` with a single untitled document.
    pub(crate) fn new() -> Self {
        Self {
            tabs: vec![DocumentState::new_untitled()],
            active_idx: 0,
        }
    }

    pub(crate) fn active_doc(&self) -> &DocumentState {
        &self.tabs[self.active_idx]
    }

    pub(crate) fn active_doc_mut(&mut self) -> &mut DocumentState {
        &mut self.tabs[self.active_idx]
    }

    /// Window title for the currently active tab.
    ///
    /// | State           | Title                  |
    /// |-----------------|------------------------|
    /// | Untitled, clean | `"Rivet"`              |
    /// | Named, clean    | `"name — Rivet"`       |
    /// | Named, dirty    | `"*name — Rivet"`      |
    /// | Untitled, dirty | `"*Untitled — Rivet"`  |
    pub(crate) fn window_title(&self) -> String {
        let doc = self.active_doc();
        if doc.path.is_none() && !doc.dirty {
            return "Rivet".to_owned();
        }
        let dirty = if doc.dirty { "*" } else { "" };
        format!("{dirty}{} \u{2014} Rivet", doc.display_name())
    }

    /// Append a new untitled tab entry and return its index.
    ///
    /// The caller must push a matching `ScintillaView` into `WindowState::sci_views`
    /// at the same index to maintain the parallel-vec invariant.
    pub(crate) fn push_untitled(&mut self) -> usize {
        self.tabs.push(DocumentState::new_untitled());
        self.tabs.len() - 1
    }

    /// Remove the tab at `idx` and adjust `active_idx`.
    ///
    /// Panics if `idx >= tabs.len()`.  The caller must remove the matching
    /// `ScintillaView` from `WindowState::sci_views` simultaneously.
    ///
    /// Returns the new `active_idx` after removal.
    pub(crate) fn remove_tab(&mut self, idx: usize) -> usize {
        self.tabs.remove(idx);
        // Clamp active_idx to the new valid range.
        if self.active_idx >= self.tabs.len() {
            self.active_idx = self.tabs.len().saturating_sub(1);
        } else if self.active_idx > idx {
            self.active_idx -= 1;
        }
        self.active_idx
    }

    /// Number of open tabs.
    pub(crate) fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    // ── File open ─────────────────────────────────────────────────────────────

    /// Update the active document state after reading `bytes` from `path`.
    ///
    /// Returns the UTF-8 content to pass to `ScintillaView::set_text`.
    pub(crate) fn open_file(&mut self, path: PathBuf, bytes: &[u8]) -> Vec<u8> {
        let doc = self.active_doc_mut();
        doc.large_file = bytes.len() as u64 > LARGE_FILE_THRESHOLD_BYTES;
        doc.dirty = false;

        let (encoding, utf8) = Self::detect_and_decode(bytes);
        doc.encoding = encoding;
        doc.eol = Self::detect_eol(&utf8);
        doc.path = Some(path);
        utf8
    }

    /// Detect encoding and transcode to UTF-8.
    fn detect_and_decode(bytes: &[u8]) -> (Encoding, Vec<u8>) {
        if bytes.starts_with(&[0xFF, 0xFE]) {
            let units: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            return (
                Encoding::Utf16Le,
                String::from_utf16_lossy(&units).into_bytes(),
            );
        }
        if bytes.starts_with(&[0xFE, 0xFF]) {
            let units: Vec<u16> = bytes[2..]
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            return (
                Encoding::Utf16Be,
                String::from_utf16_lossy(&units).into_bytes(),
            );
        }
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return (Encoding::Utf8, bytes[3..].to_vec());
        }
        if std::str::from_utf8(bytes).is_ok() {
            return (Encoding::Utf8, bytes.to_vec());
        }
        (Encoding::Ansi, bytes.to_vec())
    }

    /// Detect the dominant EOL style.
    fn detect_eol(utf8: &[u8]) -> EolMode {
        let (mut crlf, mut lf, mut cr) = (0usize, 0usize, 0usize);
        let mut i = 0;
        while i < utf8.len() {
            match utf8[i] {
                b'\r' if utf8.get(i + 1) == Some(&b'\n') => {
                    crlf += 1;
                    i += 2;
                }
                b'\r' => {
                    cr += 1;
                    i += 1;
                }
                b'\n' => {
                    lf += 1;
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
        }
        if crlf >= lf && crlf >= cr {
            EolMode::Crlf
        } else if lf >= cr {
            EolMode::Lf
        } else {
            EolMode::Cr
        }
    }

    // ── File save ─────────────────────────────────────────────────────────────

    /// Write `utf8_content` to `path` using the active document's encoding.
    ///
    /// On success, updates `active_doc().path` (for Save As) and clears
    /// `active_doc().dirty`.  The caller must call `ScintillaView::set_save_point()`.
    pub(crate) fn save(&mut self, path: PathBuf, utf8_content: &[u8]) -> crate::error::Result<()> {
        let bytes = self.encode_for_disk(utf8_content);
        std::fs::write(&path, &bytes)?;
        let doc = self.active_doc_mut();
        doc.path = Some(path);
        doc.dirty = false;
        Ok(())
    }

    fn encode_for_disk(&self, utf8: &[u8]) -> Vec<u8> {
        match self.active_doc().encoding {
            Encoding::Utf8 => utf8.to_vec(),
            Encoding::Utf16Le => {
                let s = String::from_utf8_lossy(utf8);
                let mut out = vec![0xFF_u8, 0xFE];
                for u in s.encode_utf16() {
                    out.extend_from_slice(&u.to_le_bytes());
                }
                out
            }
            Encoding::Utf16Be => {
                let s = String::from_utf8_lossy(utf8);
                let mut out = vec![0xFE_u8, 0xFF];
                for u in s.encode_utf16() {
                    out.extend_from_slice(&u.to_be_bytes());
                }
                out
            }
            Encoding::Ansi => utf8.to_vec(),
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn title_clean_untitled() {
        assert_eq!(App::new().window_title(), "Rivet");
    }

    #[test]
    fn title_clean_with_path() {
        let mut app = App::new();
        app.tabs[0].path = Some(PathBuf::from(r"C:\notes\todo.txt"));
        assert_eq!(app.window_title(), "todo.txt \u{2014} Rivet");
    }

    #[test]
    fn title_dirty_with_path() {
        let mut app = App::new();
        app.tabs[0].path = Some(PathBuf::from(r"C:\notes\todo.txt"));
        app.tabs[0].dirty = true;
        assert_eq!(app.window_title(), "*todo.txt \u{2014} Rivet");
    }

    #[test]
    fn title_dirty_untitled() {
        let mut app = App::new();
        app.tabs[0].dirty = true;
        assert_eq!(app.window_title(), "*Untitled \u{2014} Rivet");
    }

    #[test]
    fn push_and_remove_tabs() {
        let mut app = App::new();
        let i = app.push_untitled();
        assert_eq!(i, 1);
        assert_eq!(app.tab_count(), 2);
        app.active_idx = 1;
        app.remove_tab(1);
        assert_eq!(app.tab_count(), 1);
        assert_eq!(app.active_idx, 0);
    }

    #[test]
    fn detect_encoding_utf16le() {
        let bytes = b"\xFF\xFEh\x00i\x00";
        let (enc, utf8) = App::detect_and_decode(bytes);
        assert_eq!(enc, Encoding::Utf16Le);
        assert_eq!(utf8, b"hi");
    }

    #[test]
    fn detect_encoding_utf8_bom() {
        let (enc, utf8) = App::detect_and_decode(b"\xEF\xBB\xBFhello");
        assert_eq!(enc, Encoding::Utf8);
        assert_eq!(utf8, b"hello");
    }

    #[test]
    fn detect_encoding_ansi_fallback() {
        let (enc, _) = App::detect_and_decode(b"\x80\x81\x82");
        assert_eq!(enc, Encoding::Ansi);
    }

    #[test]
    fn detect_eol_crlf() {
        assert_eq!(App::detect_eol(b"a\r\nb\r\nc\n"), EolMode::Crlf);
    }

    #[test]
    fn detect_eol_lf() {
        assert_eq!(App::detect_eol(b"a\nb\nc\n"), EolMode::Lf);
    }

    #[test]
    fn encoding_roundtrip_str() {
        for enc in [
            Encoding::Utf8,
            Encoding::Utf16Le,
            Encoding::Utf16Be,
            Encoding::Ansi,
        ] {
            assert_eq!(Encoding::from_str(enc.as_str()), Some(enc));
        }
    }

    #[test]
    fn eol_roundtrip_str() {
        for eol in [EolMode::Crlf, EolMode::Lf, EolMode::Cr] {
            assert_eq!(EolMode::from_str(eol.as_str()), Some(eol));
        }
    }
}
