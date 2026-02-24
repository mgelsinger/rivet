// ── Central error type ────────────────────────────────────────────────────────
//
// All fallible operations in Rivet return `error::Result<T>`.  No panics
// in production paths; errors surface as user-facing dialogs (see
// `platform::win32::window::show_error_dialog`).

/// Every error that Rivet can produce.
#[derive(Debug)]
pub enum RivetError {
    /// A Win32 API call returned a failure code.
    Win32 {
        /// The name of the failing function, for display purposes.
        function: &'static str,
        /// The raw Win32 error code (`GetLastError()` value) or HRESULT.
        code: u32,
    },

    /// A standard I/O error (file open, read, write, …).
    Io(std::io::Error),
}

impl std::fmt::Display for RivetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Win32 { function, code } => {
                write!(f, "{function} failed (error {code:#010x})")
            }
            Self::Io(e) => write!(f, "I/O error: {e}"),
        }
    }
}

impl std::error::Error for RivetError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Win32 { .. } => None,
        }
    }
}

impl From<std::io::Error> for RivetError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// Convert a windows-crate error (HRESULT) directly into a RivetError so that
// `?` can be used on `windows::core::Result<T>` throughout the platform module.
impl From<windows::core::Error> for RivetError {
    fn from(e: windows::core::Error) -> Self {
        // HRESULT.0 is i32; reinterpret bits as u32 for display purposes.
        // Win32 errors appear as 0x8007xxxx HRESULTs.
        Self::Win32 {
            function: "windows",
            code: e.code().0 as u32,
        }
    }
}

/// Convenience alias used throughout the crate.
pub type Result<T> = std::result::Result<T, RivetError>;
