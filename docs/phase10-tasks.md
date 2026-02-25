# Phase 10 — Packaging + Release

## Scope

Three hardening and release-readiness improvements:

1. **`LoadLibraryExW` hardening** — replace the bare `LoadLibraryW("SciLexer.dll")`
   filename-only load with `LoadLibraryExW` using a full absolute path built at
   runtime from `GetModuleFileNameW`.  Eliminates DLL hijacking via CWD or PATH.
2. **Tagged GitHub Release workflow** — extend CI to build a release zip and
   publish it as a GitHub Release whenever a `v*` tag is pushed.
3. **README update** — reflect current status (v0.1.0), accurate GitHub URL,
   up-to-date project structure, and SciLexer.dll installation notes.

---

## Files changed

| File | Change |
|------|--------|
| `src/editor/scintilla/mod.rs` | Added `GetModuleFileNameW`, `LoadLibraryExW`, `LOAD_WITH_ALTERED_SEARCH_PATH`, `HANDLE`, `PWSTR` imports; rewrote `SciDll::load()` to use absolute path + `LoadLibraryExW`; updated security-note comment |
| `.github/workflows/ci.yml` | Added `tags: ["v*"]` trigger; added `release` job with packaging and `gh release create` |
| `README.md` | Updated status, GitHub URL, feature list, project structure, SciLexer.dll requirement, CI section |
| `docs/phase10-tasks.md` | **Created** — this file |

---

## How to test

- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes
- [ ] `cargo build --release` succeeds; launching `rivet.exe` (with `SciLexer.dll` alongside) works normally
- [ ] Push a `v0.1.0` tag → CI `release` job runs, creates a GitHub Release with
      `rivet-v0.1.0-windows-x64.zip` containing `rivet.exe` and `README-INSTALL.txt`
- [ ] `check` job still runs on `main` pushes (tag push also triggers it)

---

## `LoadLibraryExW` design notes

### Threat model

`LoadLibraryW("SciLexer.dll")` with a bare filename causes Windows to search
the DLL search order: application directory, then system directories, then CWD,
then PATH.  On some configurations CWD takes precedence.  An attacker who can
write a malicious `SciLexer.dll` to the CWD (common in file-manager "open with"
scenarios) can achieve DLL hijacking.

### Mitigation

1. `GetModuleFileNameW(NULL, buf, len)` — retrieves the absolute path of
   `rivet.exe` (e.g. `C:\Users\alice\AppData\Local\Programs\Rivet\rivet.exe`).
2. The filename component is stripped, leaving the directory with a trailing
   backslash: `C:\Users\alice\...\Rivet\`.
3. `"SciLexer.dll\0"` is appended to form the full path.
4. `LoadLibraryExW(full_path, NULL, LOAD_WITH_ALTERED_SEARCH_PATH)` loads
   exactly that file — no fallback search.

### Why `LOAD_WITH_ALTERED_SEARCH_PATH`

`LOAD_WITH_ALTERED_SEARCH_PATH` tells Windows to use the directory of
`lpFileName` as the starting point for any *relative-path* imports inside the
loaded DLL.  When combined with an absolute path it also suppresses the normal
DLL search path, ensuring only the exact file is loaded.  This is the
Microsoft-recommended pattern for secure DLL loading.

---

## CI release workflow notes

- **Trigger**: `push` to tags matching `v*` (e.g. `v0.1.0`, `v1.2.3`).
- **Dependency**: the `release` job runs only if the `check` job passes.
- **Permissions**: `contents: write` is required to create GitHub Releases.
- **Archive**: `rivet-{tag}-windows-x64.zip` contains `rivet.exe` and a
  plain-text `README-INSTALL.txt` explaining the SciLexer.dll requirement.
- **SciLexer.dll**: not bundled — must come from the official Scintilla
  distribution to ensure authenticity.  The install readme references
  `https://www.scintilla.org/`.
- **Release notes**: generated automatically by GitHub from PR titles and
  commits since the previous tag (`--generate-notes`).

---

## Unsafe notes

- **`GetModuleFileNameW`**: called with `HMODULE::default()` (NULL), which is
  the documented way to retrieve the current process's exe path.  The output
  buffer is 32 768 UTF-16 code units — sufficient for any Windows extended-
  length path (`\\?\...`).  Return value `0` indicates failure; we propagate
  `GetLastError()` as a `RivetError::Win32`.
- **`LoadLibraryExW`**: called with an absolute null-terminated UTF-16 path
  and `HANDLE::default()` (NULL file handle, as required by the API when
  `LOAD_WITH_ALTERED_SEARCH_PATH` is used without a file handle).  Both
  invariants are satisfied.

---

## Known limitations

- The absolute-path approach requires that `SciLexer.dll` resides in the same
  directory as `rivet.exe`.  A separate `plugins/` subdirectory or a
  system-wide install of Scintilla would need additional path-resolution logic.
- The release zip does not bundle `SciLexer.dll`.  Users must download it
  separately.  A future phase could fetch it automatically from the Scintilla
  project if a stable artifact URL becomes available.
