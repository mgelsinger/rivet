/// Rivet build script.
///
/// Phase 1 role: validate that the host targets Windows and reserve the spot
/// where Scintilla C++ compilation will be wired up (Phase 2+).
fn main() {
    // Hard gate: Rivet is Windows-only. Fail loudly on any other target
    // rather than silently producing a broken binary.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        panic!(
            "Rivet only builds for Windows \
             (CARGO_CFG_TARGET_OS = {target_os:?})"
        );
    }

    // Only re-run the build script when it changes.
    // Phase 2+ will scope this to `vendor/scintilla/` once the source tree
    // is vendored.
    println!("cargo:rerun-if-changed=build.rs");

    // ── Scintilla placeholder ─────────────────────────────────────────────────
    // Integration decision (Phase 1): SciLexer.dll (DLL-hosting approach).
    //
    // When/if we switch to a static lib, this is where the `cc` crate
    // compilation goes:
    //
    //   cc::Build::new()
    //       .cpp(true)
    //       .include("vendor/scintilla/include")
    //       .files(scintilla_sources())
    //       .compile("scintilla");
    //
    // For now, SciLexer.dll is loaded at runtime via LoadLibraryW.
}
