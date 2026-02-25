// ── Language detection ────────────────────────────────────────────────────────
//
// Maps file paths to `Language` enum values, provides SCLEX_* IDs and keyword
// lists for Scintilla.  No Win32 imports; pure Rust.

use std::path::Path;

// Import SCLEX_* constants from the scintilla messages module.
use crate::editor::scintilla::messages::{
    SCLEX_BASH, SCLEX_BATCH, SCLEX_CPP, SCLEX_CSS, SCLEX_DIFF, SCLEX_HTML, SCLEX_JSON,
    SCLEX_MAKEFILE, SCLEX_MARKDOWN, SCLEX_NULL, SCLEX_POWERSHELL, SCLEX_PROPERTIES, SCLEX_PYTHON,
    SCLEX_RUST, SCLEX_SQL, SCLEX_TOML, SCLEX_XML, SCLEX_YAML,
};

// ── Language enum ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Language {
    PlainText,
    C,
    Cpp,
    Python,
    Rust,
    JavaScript,
    TypeScript,
    Html,
    Xml,
    Css,
    Json,
    Sql,
    Toml,
    Ini,
    Batch,
    Makefile,
    Diff,
    Shell,
    Markdown,
    Yaml,
    PowerShell,
}

impl Language {
    /// Scintilla lexer ID for this language.
    pub(crate) fn lexer_id(self) -> usize {
        match self {
            Language::PlainText => SCLEX_NULL,
            Language::C => SCLEX_CPP,
            Language::Cpp => SCLEX_CPP,
            Language::JavaScript => SCLEX_CPP,
            Language::TypeScript => SCLEX_CPP,
            Language::Python => SCLEX_PYTHON,
            Language::Rust => SCLEX_RUST,
            Language::Html => SCLEX_HTML,
            Language::Xml => SCLEX_XML,
            Language::Css => SCLEX_CSS,
            Language::Json => SCLEX_JSON,
            Language::Sql => SCLEX_SQL,
            Language::Toml => SCLEX_TOML,
            Language::Ini => SCLEX_PROPERTIES,
            Language::Batch => SCLEX_BATCH,
            Language::Makefile => SCLEX_MAKEFILE,
            Language::Diff => SCLEX_DIFF,
            Language::Shell => SCLEX_BASH,
            Language::Markdown => SCLEX_MARKDOWN,
            Language::Yaml => SCLEX_YAML,
            Language::PowerShell => SCLEX_POWERSHELL,
        }
    }

    /// Human-readable name for the status bar.
    pub(crate) fn display_name(self) -> &'static str {
        match self {
            Language::PlainText => "Plain Text",
            Language::C => "C",
            Language::Cpp => "C++",
            Language::Python => "Python",
            Language::Rust => "Rust",
            Language::JavaScript => "JavaScript",
            Language::TypeScript => "TypeScript",
            Language::Html => "HTML",
            Language::Xml => "XML",
            Language::Css => "CSS",
            Language::Json => "JSON",
            Language::Sql => "SQL",
            Language::Toml => "TOML",
            Language::Ini => "INI",
            Language::Batch => "Batch",
            Language::Makefile => "Makefile",
            Language::Diff => "Diff",
            Language::Shell => "Shell",
            Language::Markdown => "Markdown",
            Language::Yaml => "YAML",
            Language::PowerShell => "PowerShell",
        }
    }
}

// ── Language detection ────────────────────────────────────────────────────────

/// Detect the language from a file path by inspecting the filename and
/// extension.  Returns `Language::PlainText` when no match is found.
pub(crate) fn language_from_path(path: &Path) -> Language {
    // Check extension-less special filenames first.
    if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
        match name {
            "Makefile" | "GNUmakefile" | "makefile" => return Language::Makefile,
            _ => {}
        }
    }

    // Match by lowercased extension.
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase());

    match ext.as_deref() {
        Some("c") | Some("h") => Language::C,
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") | Some("hh") | Some("hxx")
        | Some("inl") => Language::Cpp,
        Some("py") | Some("pyw") | Some("pyi") => Language::Python,
        Some("rs") => Language::Rust,
        Some("js") | Some("mjs") | Some("cjs") => Language::JavaScript,
        Some("ts") | Some("mts") | Some("cts") => Language::TypeScript,
        Some("html") | Some("htm") | Some("xhtml") | Some("shtml") => Language::Html,
        Some("xml") | Some("xsl") | Some("xslt") | Some("svg") | Some("xaml") | Some("csproj")
        | Some("vbproj") => Language::Xml,
        Some("css") | Some("scss") | Some("less") => Language::Css,
        Some("json") | Some("jsonc") => Language::Json,
        Some("sql") => Language::Sql,
        Some("toml") => Language::Toml,
        Some("ini") | Some("cfg") | Some("conf") | Some("properties") | Some("editorconfig") => {
            Language::Ini
        }
        Some("bat") | Some("cmd") => Language::Batch,
        Some("mk") | Some("mak") => Language::Makefile,
        Some("diff") | Some("patch") => Language::Diff,
        Some("sh") | Some("bash") | Some("zsh") | Some("ksh") | Some("ash") => Language::Shell,
        Some("md") | Some("markdown") | Some("mdown") | Some("mkd") => Language::Markdown,
        Some("yaml") | Some("yml") => Language::Yaml,
        Some("ps1") | Some("psm1") | Some("psd1") => Language::PowerShell,
        _ => Language::PlainText,
    }
}

// ── Keyword lists ─────────────────────────────────────────────────────────────

/// Returns `(keyword-set-index, null-terminated ASCII word list)` pairs for the
/// given language.  Scintilla copies the string internally so stack lifetime is
/// safe.  Languages without keyword sets return an empty slice.
pub(crate) fn keywords(lang: Language) -> &'static [(usize, &'static [u8])] {
    match lang {
        Language::C => C_KEYWORDS,
        Language::Cpp => CPP_KEYWORDS,
        Language::JavaScript => JS_KEYWORDS,
        Language::TypeScript => TS_KEYWORDS,
        Language::Python => PY_KEYWORDS,
        Language::Rust => RUST_KEYWORDS,
        Language::Sql => SQL_KEYWORDS,
        Language::PowerShell => PS_KEYWORDS,
        _ => &[],
    }
}

// ── Keyword tables ────────────────────────────────────────────────────────────

static C_KEYWORDS: &[(usize, &[u8])] = &[(
    0,
    b"auto break case char const continue default do double else enum extern \
float for goto if inline int long register restrict return short signed sizeof \
static struct switch typedef union unsigned void volatile while _Bool _Complex \
_Imaginary\0",
)];

static CPP_KEYWORDS: &[(usize, &[u8])] = &[
    (
        0,
        b"alignas alignof and and_eq asm auto bitand bitor bool break case catch char \
char8_t char16_t char32_t class compl concept const consteval constexpr constinit \
const_cast continue co_await co_return co_yield decltype default delete do double \
dynamic_cast else enum explicit export extern false float for friend goto if \
inline int long mutable namespace new noexcept not not_eq nullptr operator or \
or_eq private protected public register reinterpret_cast requires return short \
signed sizeof static static_assert static_cast struct switch template this \
thread_local throw true try typedef typeid typename union unsigned using virtual \
void volatile wchar_t while xor xor_eq\0",
    ),
    (
        1,
        b"int8_t int16_t int32_t int64_t uint8_t uint16_t uint32_t uint64_t \
size_t ssize_t ptrdiff_t intptr_t uintptr_t nullptr_t\0",
    ),
];

static JS_KEYWORDS: &[(usize, &[u8])] = &[(
    0,
    b"break case catch class const continue debugger default delete do else export \
extends false finally for function if import in instanceof let new null of return \
static super switch this throw true try typeof undefined var void while with yield \
async await\0",
)];

static TS_KEYWORDS: &[(usize, &[u8])] = &[(
    0,
    b"abstract any as async await boolean break case catch class const constructor \
continue declare default delete do else enum export extends false finally for \
from function get if implements import in infer instanceof interface is keyof \
let module namespace never new null number object of override private protected \
public readonly return set static string super switch symbol this throw true try \
type typeof undefined unique unknown var void while with yield\0",
)];

static PY_KEYWORDS: &[(usize, &[u8])] = &[(
    0,
    b"False None True and as assert async await break class continue def del elif \
else except finally for from global if import in is lambda nonlocal not or pass \
raise return try while with yield\0",
)];

static RUST_KEYWORDS: &[(usize, &[u8])] = &[
    (
        0,
        b"as async await break const continue crate dyn else enum extern false fn for \
if impl in let loop match mod move mut pub ref return self Self static struct \
super trait true type union unsafe use where while\0",
    ),
    (
        1,
        b"bool char f32 f64 i8 i16 i32 i64 i128 isize str u8 u16 u32 u64 u128 usize \
String Vec Option Result Box Rc Arc HashMap HashSet\0",
    ),
];

static SQL_KEYWORDS: &[(usize, &[u8])] = &[(
    0,
    b"ADD ALL ALTER AND AS ASC BETWEEN BY CASE CHECK COLUMN CONSTRAINT CREATE \
CROSS DATABASE DEFAULT DELETE DESC DISTINCT DROP ELSE END EXCEPT EXISTS FOREIGN \
FROM FULL GROUP HAVING IN INDEX INNER INSERT INTERSECT INTO IS JOIN KEY LEFT LIKE \
LIMIT NOT NULL ON OR ORDER OUTER PRIMARY REFERENCES RIGHT ROLLBACK SELECT SET \
TABLE TOP TRUNCATE UNION UNIQUE UPDATE VALUES VIEW WHERE WITH\0",
)];

static PS_KEYWORDS: &[(usize, &[u8])] = &[(
    0,
    b"begin break catch class continue data define do dynamicparam else elseif end \
exit filter finally for foreach from function hidden if in inlinescript parallel \
param pipeline process return sequence switch throw trap try until using var \
while workflow\0",
)];

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── language_from_path ────────────────────────────────────────────────────

    #[test]
    fn detect_rust() {
        assert_eq!(language_from_path(Path::new("main.rs")), Language::Rust);
    }

    #[test]
    fn detect_c() {
        assert_eq!(language_from_path(Path::new("util.c")), Language::C);
        assert_eq!(language_from_path(Path::new("util.h")), Language::C);
    }

    #[test]
    fn detect_cpp() {
        assert_eq!(language_from_path(Path::new("main.cpp")), Language::Cpp);
        assert_eq!(language_from_path(Path::new("main.cc")), Language::Cpp);
        assert_eq!(language_from_path(Path::new("main.hpp")), Language::Cpp);
    }

    #[test]
    fn detect_python() {
        assert_eq!(language_from_path(Path::new("script.py")), Language::Python);
        assert_eq!(language_from_path(Path::new("stub.pyi")), Language::Python);
    }

    #[test]
    fn detect_javascript() {
        assert_eq!(
            language_from_path(Path::new("app.js")),
            Language::JavaScript
        );
        assert_eq!(
            language_from_path(Path::new("mod.mjs")),
            Language::JavaScript
        );
    }

    #[test]
    fn detect_typescript() {
        assert_eq!(
            language_from_path(Path::new("app.ts")),
            Language::TypeScript
        );
        assert_eq!(
            language_from_path(Path::new("app.mts")),
            Language::TypeScript
        );
    }

    #[test]
    fn detect_html() {
        assert_eq!(language_from_path(Path::new("index.html")), Language::Html);
        assert_eq!(language_from_path(Path::new("page.htm")), Language::Html);
        assert_eq!(language_from_path(Path::new("page.xhtml")), Language::Html);
    }

    #[test]
    fn detect_xml() {
        assert_eq!(language_from_path(Path::new("data.xml")), Language::Xml);
        assert_eq!(language_from_path(Path::new("icon.svg")), Language::Xml);
    }

    #[test]
    fn detect_css() {
        assert_eq!(language_from_path(Path::new("style.css")), Language::Css);
        assert_eq!(language_from_path(Path::new("style.scss")), Language::Css);
    }

    #[test]
    fn detect_json() {
        assert_eq!(language_from_path(Path::new("config.json")), Language::Json);
        assert_eq!(
            language_from_path(Path::new("config.jsonc")),
            Language::Json
        );
    }

    #[test]
    fn detect_toml() {
        assert_eq!(language_from_path(Path::new("Cargo.toml")), Language::Toml);
    }

    #[test]
    fn detect_yaml() {
        assert_eq!(language_from_path(Path::new("ci.yml")), Language::Yaml);
        assert_eq!(language_from_path(Path::new("ci.yaml")), Language::Yaml);
    }

    #[test]
    fn detect_markdown() {
        assert_eq!(
            language_from_path(Path::new("README.md")),
            Language::Markdown
        );
        assert_eq!(
            language_from_path(Path::new("README.markdown")),
            Language::Markdown
        );
    }

    #[test]
    fn detect_shell() {
        assert_eq!(language_from_path(Path::new("install.sh")), Language::Shell);
        assert_eq!(
            language_from_path(Path::new("install.bash")),
            Language::Shell
        );
        assert_eq!(language_from_path(Path::new("rc.zsh")), Language::Shell);
    }

    #[test]
    fn detect_powershell() {
        assert_eq!(
            language_from_path(Path::new("deploy.ps1")),
            Language::PowerShell
        );
        assert_eq!(
            language_from_path(Path::new("mod.psm1")),
            Language::PowerShell
        );
    }

    #[test]
    fn detect_batch() {
        assert_eq!(language_from_path(Path::new("build.bat")), Language::Batch);
        assert_eq!(language_from_path(Path::new("run.cmd")), Language::Batch);
    }

    #[test]
    fn detect_makefile_by_name() {
        assert_eq!(
            language_from_path(Path::new("Makefile")),
            Language::Makefile
        );
        assert_eq!(
            language_from_path(Path::new("GNUmakefile")),
            Language::Makefile
        );
        assert_eq!(
            language_from_path(Path::new("makefile")),
            Language::Makefile
        );
    }

    #[test]
    fn detect_makefile_by_extension() {
        assert_eq!(
            language_from_path(Path::new("rules.mk")),
            Language::Makefile
        );
        assert_eq!(
            language_from_path(Path::new("rules.mak")),
            Language::Makefile
        );
    }

    #[test]
    fn detect_diff() {
        assert_eq!(language_from_path(Path::new("fix.diff")), Language::Diff);
        assert_eq!(language_from_path(Path::new("fix.patch")), Language::Diff);
    }

    #[test]
    fn detect_ini() {
        assert_eq!(language_from_path(Path::new("app.ini")), Language::Ini);
        assert_eq!(language_from_path(Path::new("app.cfg")), Language::Ini);
        assert_eq!(
            language_from_path(Path::new(".editorconfig")),
            Language::Ini
        );
        assert_eq!(
            language_from_path(Path::new("gradle.properties")),
            Language::Ini
        );
    }

    #[test]
    fn detect_sql() {
        assert_eq!(language_from_path(Path::new("schema.sql")), Language::Sql);
    }

    #[test]
    fn detect_plain_text_for_unknown_extension() {
        assert_eq!(
            language_from_path(Path::new("file.xyz")),
            Language::PlainText
        );
        assert_eq!(language_from_path(Path::new("no_ext")), Language::PlainText);
    }

    // Extension matching is case-insensitive.
    #[test]
    fn extension_case_insensitive() {
        assert_eq!(language_from_path(Path::new("main.RS")), Language::Rust);
        assert_eq!(language_from_path(Path::new("main.Py")), Language::Python);
        assert_eq!(language_from_path(Path::new("index.HTML")), Language::Html);
    }

    // ── display_name ─────────────────────────────────────────────────────────

    #[test]
    fn display_names_are_nonempty() {
        let langs = [
            Language::PlainText,
            Language::C,
            Language::Cpp,
            Language::Python,
            Language::Rust,
            Language::JavaScript,
            Language::TypeScript,
            Language::Html,
            Language::Xml,
            Language::Css,
            Language::Json,
            Language::Sql,
            Language::Toml,
            Language::Ini,
            Language::Batch,
            Language::Makefile,
            Language::Diff,
            Language::Shell,
            Language::Markdown,
            Language::Yaml,
            Language::PowerShell,
        ];
        for lang in langs {
            assert!(
                !lang.display_name().is_empty(),
                "{lang:?} has empty display_name"
            );
        }
    }

    // ── keywords ─────────────────────────────────────────────────────────────

    #[test]
    fn keyword_lists_are_null_terminated() {
        // Every keyword byte-slice must end with b'\0' so Scintilla reads it safely.
        let langs_with_kw = [
            Language::C,
            Language::Cpp,
            Language::JavaScript,
            Language::TypeScript,
            Language::Python,
            Language::Rust,
            Language::Sql,
            Language::PowerShell,
        ];
        for lang in langs_with_kw {
            for (_, words) in keywords(lang) {
                assert!(
                    words.last() == Some(&b'\0'),
                    "{lang:?} keyword list is not null-terminated"
                );
            }
        }
    }
}
