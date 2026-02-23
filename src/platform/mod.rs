// ── Platform abstraction layer ────────────────────────────────────────────────
//
// This module defines the public interface that the rest of the codebase uses
// to talk to the OS.  No `unsafe` lives here; all Win32 FFI is confined to the
// `win32` sub-module and never leaks outward.

pub mod win32;
