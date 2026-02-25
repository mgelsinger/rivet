# Phase 8 — UI Polish (Dark Mode + DPI Awareness)

## Scope

Two UI improvements deferred from earlier phases:

1. **Per-Monitor DPI v2** — correct scaling on high-DPI and mixed-DPI setups.
2. **Dark Mode** — VS Code Dark+-inspired palette, persisted across restarts.

---

## Files changed

| File | Change |
|------|--------|
| `Cargo.toml` | Added `Win32_UI_HiDpi`, `Win32_Graphics_Dwm` features |
| `src/platform/win32/mod.rs` | Uncommented `pub(crate) mod dpi;` |
| `src/platform/win32/dpi.rs` | **Created** — `init()`, `get_for_window()`, `get_system_dpi()`, `scale()` |
| `src/theme.rs` | Replaced 23 `CLR_*` constants with `Palette` struct + `LIGHT`/`DARK`; updated all per-lexer fns to `fn apply_X(sci, p: &Palette)`; `apply_theme` gains `dark: bool` |
| `src/platform/win32/window.rs` | DPI init; DPI-scaled window + status bar; `WM_DPICHANGED` handler; `dpi`/`dark_mode` in `WindowState`; Dark Mode menu + toggle + chrome; session integration; `apply_highlighting(dark)` |
| `src/session/mod.rs` | `dark_mode: bool` field on `SessionFile`; `dark_mode` param on `save()` |
| `src/main.rs` | Updated module comment |
| `docs/phase8-tasks.md` | **Created** — this file |

---

## How to test

- [ ] View > Dark Mode toggles dark Scintilla background + syntax colours on all open tabs
- [ ] View > Dark Mode checkmark reflects current state
- [ ] Window title bar / chrome turns dark (Win10 1903+)
- [ ] File > New while in dark mode → new tab gets dark theme immediately
- [ ] Dark mode preference survives app restart (stored in `session.json`)
- [ ] At 150%/200% DPI: tab bar height scales proportionally
- [ ] At 150%/200% DPI: status bar parts don't clip text
- [ ] Move window to a different-DPI monitor → window rescales correctly
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo test` passes

---

## Unsafe notes

- **`dpi.rs`**: `SetProcessDpiAwarenessContext`, `GetDpiForWindow`, `GetDpiForSystem`
  — all called with correct invariants; live in `platform::win32` where unsafe is
  permitted per the project policy.
- **`apply_title_bar_dark`**: `DwmSetWindowAttribute` — `pvAttribute` is a valid
  `*const u32`; `cbAttribute` matches `size_of::<u32>()`.
- **`WM_DPICHANGED` handler**: `LPARAM` is guaranteed by Windows to be a valid
  `*const RECT` for this message.

---

## Known limitations

- The tab strip (`SysTabControl32`) and status bar (`msctls_statusbar32`) do not
  auto-darken in dark mode. They retain the system light appearance. This is a
  known Win32 limitation; addressing it would require custom owner-draw or
  `WM_CTLCOLOR*` interception (Phase 9 candidate).
- The `#[serde(default)]` attribute on `dark_mode` ensures old `session.json`
  files (without the field) parse successfully and default to light mode.
- The `open_untitled_tab` fix: Phase 7 accidentally omitted `apply_highlighting`
  for new untitled tabs. Phase 8 adds it so Consolas font and the correct palette
  are applied consistently regardless of how a tab is created.
