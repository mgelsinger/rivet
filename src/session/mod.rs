// ── Session persistence ───────────────────────────────────────────────────────
//
// Reads and writes `%APPDATA%\Rivet\session.json`.
// No `unsafe` — pure safe Rust + serde_json.

use std::{fs, io, path::PathBuf};

use serde::{Deserialize, Serialize};

// ── On-disk types ─────────────────────────────────────────────────────────────

/// Root of the JSON session file.
#[derive(Serialize, Deserialize)]
pub(crate) struct SessionFile {
    pub(crate) version:    u32,
    pub(crate) tabs:       Vec<TabEntry>,
    pub(crate) active_tab: usize,
}

/// One entry per open tab.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct TabEntry {
    /// Absolute path to the file, or `None` for untitled buffers.
    pub(crate) path:        Option<String>,
    /// Raw byte offset of the caret (`SCI_GETCURRENTPOS`).
    pub(crate) caret_pos:   usize,
    /// First visible line (`SCI_GETFIRSTVISIBLELINE`).
    pub(crate) scroll_line: usize,
    /// Encoding label, e.g. `"UTF-8"`.
    pub(crate) encoding:    String,
    /// EOL label, e.g. `"CRLF"`.
    pub(crate) eol:         String,
}

// ── Format version ────────────────────────────────────────────────────────────

const SESSION_VERSION: u32 = 1;

// ── Path ──────────────────────────────────────────────────────────────────────

/// Return the path to the session file: `%APPDATA%\Rivet\session.json`.
///
/// Returns `None` if the `APPDATA` environment variable is not set.
pub(crate) fn session_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let mut p = PathBuf::from(appdata);
    p.push("Rivet");
    p.push("session.json");
    Some(p)
}

// ── Save ──────────────────────────────────────────────────────────────────────

/// Write the session to `%APPDATA%\Rivet\session.json`.
///
/// Creates the `Rivet` directory if it does not exist.
/// The caller (`window.rs`) silently discards any returned error.
pub(crate) fn save(tabs: &[TabEntry], active_tab: usize) -> io::Result<()> {
    let path = session_path()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA not set"))?;

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let sf = SessionFile {
        version:    SESSION_VERSION,
        tabs:       tabs.to_vec(),
        active_tab,
    };

    let file = fs::File::create(&path)?;
    serde_json::to_writer_pretty(file, &sf)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))
}

// ── Load ──────────────────────────────────────────────────────────────────────

/// Read and parse the session file.
///
/// Returns `None` on any error: file missing, JSON parse failure, or an
/// unrecognised version number.  The app continues with a fresh untitled tab.
pub(crate) fn load() -> Option<SessionFile> {
    let path = session_path()?;
    let data = fs::read(&path).ok()?;
    let sf: SessionFile = serde_json::from_slice(&data).ok()?;
    if sf.version != SESSION_VERSION {
        return None;
    }
    Some(sf)
}
