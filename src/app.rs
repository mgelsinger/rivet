// ── Application lifecycle & top-level state ────────────────────────────────────
//
// A single `App` is created on startup and owned by `WindowState` for the
// lifetime of the main window.  All mutations happen on the UI thread — there
// is no global mutable state.

use std::path::PathBuf;

use crate::editor::LARGE_FILE_THRESHOLD_BYTES;

// ── Encoding ──────────────────────────────────────────────────────────────────

/// The character encoding of the document on disk.
///
/// Rivet always keeps the in-memory representation as UTF-8 (Scintilla's
/// native mode).  This field records what encoding should be used when
/// writing back to disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Encoding {
    /// UTF-8, with or without BOM.
    Utf8,
    /// UTF-16 Little-Endian with BOM.
    Utf16Le,
    /// UTF-16 Big-Endian with BOM.
    Utf16Be,
    /// System ANSI code page (CP1252 on most Western Windows installs).
    /// Bytes are loaded into Scintilla as-is; Scintilla treats them as Latin-1.
    Ansi,
}

impl Encoding {
    /// Short display string shown in the status bar.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Ansi => "ANSI",
        }
    }
}

// ── EOL mode ──────────────────────────────────────────────────────────────────

/// The end-of-line convention used by the document.
///
/// Matches Scintilla's `SC_EOL_*` constants (set via `SCI_SETEOLMODE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EolMode {
    /// Windows-style `\r\n` (Scintilla: `SC_EOL_CRLF = 0`).
    Crlf,
    /// Unix-style `\n` (Scintilla: `SC_EOL_LF = 1`).
    Lf,
    /// Old Mac-style `\r` (Scintilla: `SC_EOL_CR = 2`).
    Cr,
}

impl EolMode {
    /// Short display string shown in the status bar.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Crlf => "CRLF",
            Self::Lf => "LF",
            Self::Cr => "CR",
        }
    }
}

// ── DocumentState ─────────────────────────────────────────────────────────────

/// Per-document state for the currently open file.
///
/// Phase 3 tracks one document at a time.  Phase 4 (tabs) will move this
/// into a `Vec<DocumentState>` with an active-index.
#[derive(Debug)]
pub(crate) struct DocumentState {
    /// Absolute path to the file on disk, or `None` for an untitled buffer.
    pub(crate) path: Option<PathBuf>,
    /// The encoding used to read (and that will be used to write) the file.
    pub(crate) encoding: Encoding,
    /// The EOL convention detected in the file.
    pub(crate) eol: EolMode,
    /// `true` when the buffer contains changes not yet saved to disk.
    pub(crate) dirty: bool,
    /// `true` when the file was larger than `LARGE_FILE_THRESHOLD_BYTES`.
    pub(crate) large_file: bool,
}

impl DocumentState {
    /// A fresh, untitled document with sensible defaults.
    fn new_untitled() -> Self {
        Self {
            path: None,
            encoding: Encoding::Utf8,
            eol: EolMode::Crlf,
            dirty: false,
            large_file: false,
        }
    }

    /// The bare filename component, or `"Untitled"` if no path is set.
    fn display_name(&self) -> String {
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
/// Passed by mutable reference through WndProc handlers so that all
/// application logic sees a single, explicit state root rather than a
/// collection of disconnected globals.
pub(crate) struct App {
    /// State of the currently open document.
    pub(crate) doc: DocumentState,
}

impl App {
    /// Create a fresh `App` with an untitled, empty document.
    pub(crate) fn new() -> Self {
        Self {
            doc: DocumentState::new_untitled(),
        }
    }

    /// Compute the title string for the main window.
    ///
    /// | State | Title |
    /// |---|---|
    /// | No path, clean | `"Rivet"` |
    /// | Path set, clean | `"filename — Rivet"` |
    /// | Path set, dirty | `"*filename — Rivet"` |
    /// | No path, dirty | `"*Untitled — Rivet"` |
    pub(crate) fn window_title(&self) -> String {
        let name = self.doc.display_name();
        // Untitled + clean → bare app name (startup state)
        if self.doc.path.is_none() && !self.doc.dirty {
            return "Rivet".to_owned();
        }
        let dirty = if self.doc.dirty { "*" } else { "" };
        format!("{dirty}{name} \u{2014} Rivet") // — is U+2014 EM DASH
    }

    // ── File save ─────────────────────────────────────────────────────────────

    /// Write the document to `path` using the document's current encoding.
    ///
    /// On success, updates `doc.path` (for Save As) and clears `doc.dirty`.
    /// The caller is responsible for calling `ScintillaView::set_save_point()`
    /// to synchronise Scintilla's internal dirty model.
    pub(crate) fn save(&mut self, path: std::path::PathBuf, utf8_content: &[u8]) -> crate::error::Result<()> {
        let bytes = self.encode_for_disk(utf8_content);
        std::fs::write(&path, &bytes)?;
        self.doc.path = Some(path);
        self.doc.dirty = false;
        Ok(())
    }

    /// Re-encode UTF-8 content to the document's on-disk encoding.
    fn encode_for_disk(&self, utf8: &[u8]) -> Vec<u8> {
        match self.doc.encoding {
            Encoding::Utf8 => utf8.to_vec(),
            Encoding::Utf16Le => {
                let s = String::from_utf8_lossy(utf8);
                let mut out = vec![0xFF_u8, 0xFE]; // LE BOM
                for unit in s.encode_utf16() {
                    out.extend_from_slice(&unit.to_le_bytes());
                }
                out
            }
            Encoding::Utf16Be => {
                let s = String::from_utf8_lossy(utf8);
                let mut out = vec![0xFE_u8, 0xFF]; // BE BOM
                for unit in s.encode_utf16() {
                    out.extend_from_slice(&unit.to_be_bytes());
                }
                out
            }
            // ANSI: pass bytes through as-is (Scintilla stores them verbatim).
            Encoding::Ansi => utf8.to_vec(),
        }
    }

    // ── File open ─────────────────────────────────────────────────────────────

    /// Update document state after a successful file open.
    ///
    /// Returns the bytes that should be passed to `ScintillaView::set_text`:
    /// always UTF-8 regardless of the file's on-disk encoding.
    ///
    /// Encoding detection order:
    /// 1. UTF-16 LE BOM (`FF FE`)
    /// 2. UTF-16 BE BOM (`FE FF`)
    /// 3. UTF-8 BOM (`EF BB BF`)
    /// 4. Heuristic: if the bytes are valid UTF-8, treat as UTF-8
    /// 5. Fallback: ANSI (bytes loaded as-is; Scintilla interprets as Latin-1)
    pub(crate) fn open_file(&mut self, path: PathBuf, bytes: &[u8]) -> Vec<u8> {
        self.doc.large_file = bytes.len() as u64 > LARGE_FILE_THRESHOLD_BYTES;
        self.doc.dirty = false;

        let (encoding, utf8_bytes) = Self::detect_and_decode(bytes);
        self.doc.encoding = encoding;

        // Detect dominant EOL from the decoded bytes.
        self.doc.eol = Self::detect_eol(&utf8_bytes);

        self.doc.path = Some(path);
        utf8_bytes
    }

    /// Detect the encoding of `bytes` and return the encoding + UTF-8 content.
    fn detect_and_decode(bytes: &[u8]) -> (Encoding, Vec<u8>) {
        // UTF-16 LE BOM: FF FE
        if bytes.starts_with(&[0xFF, 0xFE]) {
            let payload = &bytes[2..];
            let units: Vec<u16> = payload
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let utf8 = String::from_utf16_lossy(&units).into_bytes();
            return (Encoding::Utf16Le, utf8);
        }

        // UTF-16 BE BOM: FE FF
        if bytes.starts_with(&[0xFE, 0xFF]) {
            let payload = &bytes[2..];
            let units: Vec<u16> = payload
                .chunks_exact(2)
                .map(|c| u16::from_be_bytes([c[0], c[1]]))
                .collect();
            let utf8 = String::from_utf16_lossy(&units).into_bytes();
            return (Encoding::Utf16Be, utf8);
        }

        // UTF-8 BOM: EF BB BF
        if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
            return (Encoding::Utf8, bytes[3..].to_vec());
        }

        // Heuristic: valid UTF-8
        if std::str::from_utf8(bytes).is_ok() {
            return (Encoding::Utf8, bytes.to_vec());
        }

        // Fallback: ANSI — load as-is
        (Encoding::Ansi, bytes.to_vec())
    }

    /// Detect the dominant EOL style in UTF-8 text.
    ///
    /// Scans for `\r\n`, `\r`, and `\n` and returns whichever appears most.
    /// Falls back to `EolMode::Crlf` when no line endings are present.
    fn detect_eol(utf8: &[u8]) -> EolMode {
        let mut crlf = 0usize;
        let mut lf = 0usize;
        let mut cr = 0usize;
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
                _ => i += 1,
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
        app.doc.path = Some(PathBuf::from(r"C:\notes\todo.txt"));
        assert_eq!(app.window_title(), "todo.txt \u{2014} Rivet");
    }

    #[test]
    fn title_dirty_with_path() {
        let mut app = App::new();
        app.doc.path = Some(PathBuf::from(r"C:\notes\todo.txt"));
        app.doc.dirty = true;
        assert_eq!(app.window_title(), "*todo.txt \u{2014} Rivet");
    }

    #[test]
    fn title_dirty_untitled() {
        let mut app = App::new();
        app.doc.dirty = true;
        assert_eq!(app.window_title(), "*Untitled \u{2014} Rivet");
    }

    #[test]
    fn encoding_display() {
        assert_eq!(Encoding::Utf8.as_str(), "UTF-8");
        assert_eq!(Encoding::Utf16Le.as_str(), "UTF-16 LE");
        assert_eq!(Encoding::Utf16Be.as_str(), "UTF-16 BE");
        assert_eq!(Encoding::Ansi.as_str(), "ANSI");
    }

    #[test]
    fn eol_display() {
        assert_eq!(EolMode::Crlf.as_str(), "CRLF");
        assert_eq!(EolMode::Lf.as_str(), "LF");
        assert_eq!(EolMode::Cr.as_str(), "CR");
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
        let bytes = b"\xEF\xBB\xBFhello";
        let (enc, utf8) = App::detect_and_decode(bytes);
        assert_eq!(enc, Encoding::Utf8);
        assert_eq!(utf8, b"hello");
    }

    #[test]
    fn detect_encoding_utf8_no_bom() {
        let (enc, _) = App::detect_and_decode(b"hello world");
        assert_eq!(enc, Encoding::Utf8);
    }

    #[test]
    fn detect_encoding_ansi_fallback() {
        // 0x80–0x9F are invalid UTF-8 lead bytes
        let (enc, _) = App::detect_and_decode(b"\x80\x81\x82");
        assert_eq!(enc, Encoding::Ansi);
    }

    #[test]
    fn detect_eol_crlf_dominant() {
        let eol = App::detect_eol(b"a\r\nb\r\nc\n");
        assert_eq!(eol, EolMode::Crlf);
    }

    #[test]
    fn detect_eol_lf_dominant() {
        let eol = App::detect_eol(b"a\nb\nc\n");
        assert_eq!(eol, EolMode::Lf);
    }

    #[test]
    fn detect_eol_no_newlines_defaults_crlf() {
        let eol = App::detect_eol(b"no newlines here");
        assert_eq!(eol, EolMode::Crlf);
    }
}
