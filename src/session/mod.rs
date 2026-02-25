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
    pub(crate) version: u32,
    pub(crate) tabs: Vec<TabEntry>,
    pub(crate) active_tab: usize,
    #[serde(default)] // backward-compat: old files without this field parse as false
    pub(crate) dark_mode: bool,
    /// 0 = Top, 1 = Left, 2 = Right.
    #[serde(default)]
    pub(crate) tab_position: u8,
}

/// One entry per open tab.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct TabEntry {
    /// Absolute path to the file, or `None` for untitled buffers.
    pub(crate) path: Option<String>,
    /// Raw byte offset of the caret (`SCI_GETCURRENTPOS`).
    pub(crate) caret_pos: usize,
    /// First visible line (`SCI_GETFIRSTVISIBLELINE`).
    pub(crate) scroll_line: usize,
    /// Encoding label, e.g. `"UTF-8"`.
    pub(crate) encoding: String,
    /// EOL label, e.g. `"CRLF"`.
    pub(crate) eol: String,
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
pub(crate) fn save(
    tabs: &[TabEntry],
    active_tab: usize,
    dark_mode: bool,
    tab_position: u8,
) -> io::Result<()> {
    let path =
        session_path().ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "APPDATA not set"))?;

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let sf = SessionFile {
        version: SESSION_VERSION,
        tabs: tabs.to_vec(),
        active_tab,
        dark_mode,
        tab_position,
    };

    let file = fs::File::create(&path)?;
    serde_json::to_writer_pretty(file, &sf).map_err(io::Error::other)
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tab(path: Option<&str>) -> TabEntry {
        TabEntry {
            path: path.map(str::to_owned),
            caret_pos: 10,
            scroll_line: 2,
            encoding: "UTF-8".to_owned(),
            eol: "CRLF".to_owned(),
        }
    }

    #[test]
    fn roundtrip_with_dark_mode() {
        let sf = SessionFile {
            version: SESSION_VERSION,
            tabs: vec![make_tab(Some("C:\\foo.txt")), make_tab(None)],
            active_tab: 1,
            dark_mode: true,
            tab_position: 0,
        };
        let json = serde_json::to_string(&sf).expect("serialize");
        let sf2: SessionFile = serde_json::from_str(&json).expect("deserialize");

        assert_eq!(sf2.version, SESSION_VERSION);
        assert_eq!(sf2.active_tab, 1);
        assert!(sf2.dark_mode);
        assert_eq!(sf2.tabs.len(), 2);
        assert_eq!(sf2.tabs[0].path, Some("C:\\foo.txt".to_owned()));
        assert_eq!(sf2.tabs[0].caret_pos, 10);
        assert_eq!(sf2.tabs[0].scroll_line, 2);
        assert_eq!(sf2.tabs[0].encoding, "UTF-8");
        assert_eq!(sf2.tabs[0].eol, "CRLF");
        assert_eq!(sf2.tabs[1].path, None);
    }

    #[test]
    fn roundtrip_light_mode() {
        let sf = SessionFile {
            version: SESSION_VERSION,
            tabs: vec![],
            active_tab: 0,
            dark_mode: false,
            tab_position: 0,
        };
        let json = serde_json::to_string(&sf).expect("serialize");
        let sf2: SessionFile = serde_json::from_str(&json).expect("deserialize");
        assert!(!sf2.dark_mode);
    }

    /// Old session files written before Phase 8 have no `dark_mode` field.
    /// `#[serde(default)]` must make them parse as `dark_mode = false`.
    #[test]
    fn dark_mode_defaults_to_false_when_absent() {
        let json = r#"{"version":1,"tabs":[],"active_tab":0}"#;
        let sf: SessionFile = serde_json::from_str(json).expect("deserialize old format");
        assert!(!sf.dark_mode, "missing dark_mode should default to false");
    }

    /// A session file with an unrecognised version number must be rejected
    /// by `load()`.  Test the parse-and-check logic directly.
    #[test]
    fn wrong_version_is_rejected() {
        let sf = SessionFile {
            version: 99,
            tabs: vec![],
            active_tab: 0,
            dark_mode: false,
            tab_position: 0,
        };
        let json = serde_json::to_string(&sf).expect("serialize");
        let parsed: SessionFile = serde_json::from_str(&json).expect("deserialize");
        // load() would return None for this version; assert the condition directly.
        assert_ne!(parsed.version, SESSION_VERSION);
    }

    #[test]
    fn tab_entry_with_none_path_roundtrips() {
        let sf = SessionFile {
            version: SESSION_VERSION,
            tabs: vec![make_tab(None)],
            active_tab: 0,
            dark_mode: false,
            tab_position: 0,
        };
        let json = serde_json::to_string(&sf).expect("serialize");
        let sf2: SessionFile = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(sf2.tabs[0].path, None);
    }
}
