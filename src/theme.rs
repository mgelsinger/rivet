// ── Dual light/dark colour theme ───────────────────────────────────────────────
//
// Applies a light or dark theme to a Scintilla view for the given language.
// Call `apply_theme(sci, language, dark)` with `dark = true` for VS Code
// Dark+-inspired colours, or `dark = false` for the Notepad++-style light theme.
//
// Colour conventions:
//   • All palette entries are in 0xRRGGBB form.
//   • The `rgb!` macro converts to Scintilla's BGR COLORREF before passing to
//     the API.

use crate::{
    editor::scintilla::{
        messages::{
            SCE_BAT_COMMAND,
            // SCLEX_BATCH token styles
            SCE_BAT_COMMENT,
            SCE_BAT_LABEL,
            SCE_BAT_OPERATOR,
            SCE_BAT_WORD,
            SCE_CSS_CLASS,
            SCE_CSS_COMMENT,
            SCE_CSS_DOUBLESTRING,
            SCE_CSS_ID,
            SCE_CSS_IDENTIFIER,
            SCE_CSS_IMPORTANT,
            SCE_CSS_OPERATOR,
            SCE_CSS_PSEUDOCLASS,
            SCE_CSS_SINGLESTRING,
            // SCLEX_CSS token styles
            SCE_CSS_TAG,
            SCE_CSS_VALUE,
            SCE_C_CHARACTER,
            // SCLEX_CPP token styles
            SCE_C_COMMENT,
            SCE_C_COMMENTDOC,
            SCE_C_COMMENTLINE,
            SCE_C_NUMBER,
            SCE_C_OPERATOR,
            SCE_C_PREPROCESSOR,
            SCE_C_REGEX,
            SCE_C_STRING,
            SCE_C_WORD,
            SCE_C_WORD2,
            SCE_DIFF_ADDED,
            SCE_DIFF_COMMAND,
            // SCLEX_DIFF token styles
            SCE_DIFF_COMMENT,
            SCE_DIFF_DELETED,
            SCE_DIFF_HEADER,
            SCE_DIFF_POSITION,
            SCE_H_ATTRIBUTE,
            SCE_H_COMMENT,
            SCE_H_DOUBLESTRING,
            SCE_H_SINGLESTRING,
            // SCLEX_HTML / SCLEX_XML token styles
            SCE_H_TAG,
            SCE_H_TAGEND,
            SCE_JSON_KEYWORD,
            // SCLEX_JSON token styles
            SCE_JSON_NUMBER,
            SCE_JSON_OPERATOR,
            SCE_JSON_PROPERTYNAME,
            SCE_JSON_STRING,
            // SCLEX_MAKEFILE token styles
            SCE_MAKE_COMMENT,
            SCE_MAKE_OPERATOR,
            SCE_MAKE_PREPROCESSOR,
            SCE_MAKE_TARGET,
            SCE_MARKDOWN_BLOCKQUOTE,
            SCE_MARKDOWN_CODE,
            SCE_MARKDOWN_CODE2,
            SCE_MARKDOWN_CODEBK,
            SCE_MARKDOWN_EM1,
            SCE_MARKDOWN_EM2,
            SCE_MARKDOWN_HEADER1,
            SCE_MARKDOWN_HEADER2,
            SCE_MARKDOWN_HEADER3,
            SCE_MARKDOWN_HEADER4,
            SCE_MARKDOWN_HEADER5,
            SCE_MARKDOWN_HEADER6,
            SCE_MARKDOWN_HRULE,
            SCE_MARKDOWN_LINK,
            SCE_MARKDOWN_OLIST_ITEM,
            SCE_MARKDOWN_STRIKEOUT,
            // SCLEX_MARKDOWN token styles
            SCE_MARKDOWN_STRONG1,
            SCE_MARKDOWN_STRONG2,
            SCE_MARKDOWN_ULIST_ITEM,
            SCE_POWERSHELL_CHARACTER,
            SCE_POWERSHELL_CMDLET,
            // SCLEX_POWERSHELL token styles
            SCE_POWERSHELL_COMMENT,
            SCE_POWERSHELL_COMMENTSTREAM,
            SCE_POWERSHELL_FUNCTION,
            SCE_POWERSHELL_HERE_CHARACTER,
            SCE_POWERSHELL_HERE_STRING,
            SCE_POWERSHELL_KEYWORD,
            SCE_POWERSHELL_NUMBER,
            SCE_POWERSHELL_OPERATOR,
            SCE_POWERSHELL_STRING,
            SCE_POWERSHELL_VARIABLE,
            // SCLEX_PROPERTIES token styles
            SCE_PROPS_COMMENT,
            SCE_PROPS_KEY,
            SCE_PROPS_SECTION,
            SCE_P_CHARACTER,
            SCE_P_CLASSNAME,
            // SCLEX_PYTHON token styles
            SCE_P_COMMENTLINE,
            SCE_P_DECORATOR,
            SCE_P_DEFNAME,
            SCE_P_NUMBER,
            SCE_P_OPERATOR,
            SCE_P_STRING,
            SCE_P_TRIPLE,
            SCE_P_TRIPLEDOUBLE,
            SCE_P_WORD,
            SCE_RUST_CHARACTER,
            // SCLEX_RUST token styles
            SCE_RUST_COMMENTBLOCK,
            SCE_RUST_COMMENTBLOCKDOC,
            SCE_RUST_COMMENTLINE,
            SCE_RUST_COMMENTLINEDOC,
            SCE_RUST_LIFETIME,
            SCE_RUST_MACRO,
            SCE_RUST_NUMBER,
            SCE_RUST_OPERATOR,
            SCE_RUST_STRING,
            SCE_RUST_STRINGR,
            SCE_RUST_WORD,
            SCE_RUST_WORD2,
            SCE_SH_CHARACTER,
            // SCLEX_BASH token styles
            SCE_SH_COMMENTLINE,
            SCE_SH_NUMBER,
            SCE_SH_OPERATOR,
            SCE_SH_SCALAR,
            SCE_SH_STRING,
            SCE_SH_WORD,
            SCE_SQL_CHARACTER,
            // SCLEX_SQL token styles
            SCE_SQL_COMMENT,
            SCE_SQL_COMMENTDOC,
            SCE_SQL_COMMENTLINE,
            SCE_SQL_NUMBER,
            SCE_SQL_OPERATOR,
            SCE_SQL_STRING,
            SCE_SQL_WORD,
            SCE_TOML_BOOL,
            // SCLEX_TOML token styles
            SCE_TOML_COMMENT,
            SCE_TOML_KEY,
            SCE_TOML_NUMBER,
            SCE_TOML_SECTIONTITLE,
            SCE_TOML_STRING,
            SCE_TOML_STRINGMULTILINE,
            // SCLEX_YAML token styles
            SCE_YAML_COMMENT,
            SCE_YAML_DOCUMENT,
            SCE_YAML_IDENTIFIER,
            SCE_YAML_KEYWORD,
            SCE_YAML_NUMBER,
            SCE_YAML_OPERATOR,
            SCE_YAML_TEXT,
            STYLE_DEFAULT,
            STYLE_LINENUMBER,
        },
        ScintillaView,
    },
    languages::Language,
};

// ── Colour macro ──────────────────────────────────────────────────────────────

/// Convert 0xRRGGBB → Scintilla's BGR COLORREF.
macro_rules! rgb {
    ($r:expr, $g:expr, $b:expr) => {
        (($b as u32) << 16) | (($g as u32) << 8) | ($r as u32)
    };
}

// ── Colour palette ────────────────────────────────────────────────────────────

struct Palette {
    bg: u32,
    fg: u32,
    line_num_bg: u32,
    line_num_fg: u32,
    comment: u32,
    keyword: u32,
    keyword2: u32,
    string: u32,
    number: u32,
    preproc: u32,
    operator: u32,
    label: u32,
    regex: u32,
    tag: u32,
    attr: u32,
    section: u32,
    key: u32,
    diff_add: u32,
    diff_del: u32,
    diff_hdr: u32,
    md_header: u32,
    md_code: u32,
    yaml_key: u32,
}

/// Notepad++-style light palette.
const LIGHT: Palette = Palette {
    bg: rgb!(0xFF, 0xFF, 0xFF),
    fg: rgb!(0x00, 0x00, 0x00),
    line_num_bg: rgb!(0xE4, 0xE4, 0xE4),
    line_num_fg: rgb!(0x80, 0x80, 0x80),
    comment: rgb!(0x00, 0x80, 0x00),
    keyword: rgb!(0x00, 0x00, 0xFF),
    keyword2: rgb!(0x00, 0x00, 0x80),
    string: rgb!(0x80, 0x00, 0x00),
    number: rgb!(0xFF, 0x80, 0x00),
    preproc: rgb!(0x80, 0x40, 0x00),
    operator: rgb!(0x00, 0x00, 0x00),
    label: rgb!(0x80, 0x00, 0x80),
    regex: rgb!(0x00, 0x80, 0x80),
    tag: rgb!(0x80, 0x00, 0x00),
    attr: rgb!(0xFF, 0x00, 0x00),
    section: rgb!(0x00, 0x00, 0x80),
    key: rgb!(0x80, 0x40, 0x00),
    diff_add: rgb!(0x00, 0x80, 0x00),
    diff_del: rgb!(0x80, 0x00, 0x00),
    diff_hdr: rgb!(0x00, 0x00, 0xFF),
    md_header: rgb!(0x00, 0x00, 0x80),
    md_code: rgb!(0x80, 0x40, 0x00),
    yaml_key: rgb!(0x00, 0x00, 0x80),
};

/// VS Code Dark+-inspired dark palette.
const DARK: Palette = Palette {
    bg: rgb!(0x1E, 0x1E, 0x1E),
    fg: rgb!(0xD4, 0xD4, 0xD4),
    line_num_bg: rgb!(0x25, 0x25, 0x26),
    line_num_fg: rgb!(0x85, 0x85, 0x85),
    comment: rgb!(0x6A, 0x99, 0x55),
    keyword: rgb!(0x56, 0x9C, 0xD6),
    keyword2: rgb!(0x4E, 0xC9, 0xB0),
    string: rgb!(0xCE, 0x91, 0x78),
    number: rgb!(0xB5, 0xCE, 0xA8),
    preproc: rgb!(0xC5, 0x86, 0xC0),
    operator: rgb!(0xD4, 0xD4, 0xD4),
    label: rgb!(0x9C, 0xDC, 0xFE),
    regex: rgb!(0xD1, 0x69, 0x69),
    tag: rgb!(0x4E, 0xC9, 0xB0),
    attr: rgb!(0x9C, 0xDC, 0xFE),
    section: rgb!(0x56, 0x9C, 0xD6),
    key: rgb!(0x9C, 0xDC, 0xFE),
    diff_add: rgb!(0x6A, 0x99, 0x55),
    diff_del: rgb!(0xCE, 0x91, 0x78),
    diff_hdr: rgb!(0x56, 0x9C, 0xD6),
    md_header: rgb!(0x56, 0x9C, 0xD6),
    md_code: rgb!(0xCE, 0x91, 0x78),
    yaml_key: rgb!(0x9C, 0xDC, 0xFE),
};

// ── Public entry point ────────────────────────────────────────────────────────

/// Apply a light or dark theme to `sci` for the given `language`.
///
/// When `dark` is `true` the VS Code Dark+-inspired palette is used; when
/// `false` the Notepad++-style light palette is used.
///
/// Sequence:
/// 1. Set `STYLE_DEFAULT` font, size, and colours.
/// 2. Call `style_clear_all` to clone those into all 256 slots.
/// 3. Override `STYLE_LINENUMBER`.
/// 4. Dispatch to the per-lexer function to set token colours.
pub(crate) fn apply_theme(sci: &ScintillaView, language: Language, dark: bool) {
    let p = if dark { &DARK } else { &LIGHT };
    apply_default_styles(sci, p);
    match language {
        Language::PlainText => { /* defaults only */ }
        Language::C | Language::Cpp | Language::JavaScript | Language::TypeScript => {
            apply_cpp_theme(sci, p)
        }
        Language::Python => apply_python_theme(sci, p),
        Language::Rust => apply_rust_theme(sci, p),
        Language::Html | Language::Xml => apply_html_theme(sci, p),
        Language::Css => apply_css_theme(sci, p),
        Language::Json => apply_json_theme(sci, p),
        Language::Sql => apply_sql_theme(sci, p),
        Language::Toml => apply_toml_theme(sci, p),
        Language::Ini => apply_ini_theme(sci, p),
        Language::Batch => apply_batch_theme(sci, p),
        Language::Makefile => apply_makefile_theme(sci, p),
        Language::Diff => apply_diff_theme(sci, p),
        Language::Shell => apply_shell_theme(sci, p),
        Language::Markdown => apply_markdown_theme(sci, p),
        Language::Yaml => apply_yaml_theme(sci, p),
        Language::PowerShell => apply_powershell_theme(sci, p),
    }
}

// ── Default styles ────────────────────────────────────────────────────────────

fn apply_default_styles(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(STYLE_DEFAULT, p.fg);
    sci.style_set_back(STYLE_DEFAULT, p.bg);
    sci.style_set_font(STYLE_DEFAULT, b"Consolas\0");
    sci.style_set_size(STYLE_DEFAULT, 10);
    // Clone STYLE_DEFAULT into all 256 slots — must come BEFORE per-token overrides.
    sci.style_clear_all();
    // Override line-number margin colours.
    sci.style_set_fore(STYLE_LINENUMBER, p.line_num_fg);
    sci.style_set_back(STYLE_LINENUMBER, p.line_num_bg);
}

// ── Per-lexer theme functions ─────────────────────────────────────────────────

fn apply_cpp_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_C_COMMENT, p.comment);
    sci.style_set_fore(SCE_C_COMMENTLINE, p.comment);
    sci.style_set_fore(SCE_C_COMMENTDOC, p.comment);
    sci.style_set_fore(SCE_C_NUMBER, p.number);
    sci.style_set_fore(SCE_C_WORD, p.keyword);
    sci.style_set_bold(SCE_C_WORD, true);
    sci.style_set_fore(SCE_C_WORD2, p.keyword2);
    sci.style_set_fore(SCE_C_STRING, p.string);
    sci.style_set_fore(SCE_C_CHARACTER, p.string);
    sci.style_set_fore(SCE_C_PREPROCESSOR, p.preproc);
    sci.style_set_fore(SCE_C_OPERATOR, p.operator);
    sci.style_set_fore(SCE_C_REGEX, p.regex);
}

fn apply_python_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_P_COMMENTLINE, p.comment);
    sci.style_set_fore(SCE_P_NUMBER, p.number);
    sci.style_set_fore(SCE_P_STRING, p.string);
    sci.style_set_fore(SCE_P_CHARACTER, p.string);
    sci.style_set_fore(SCE_P_TRIPLE, p.comment);
    sci.style_set_fore(SCE_P_TRIPLEDOUBLE, p.comment);
    sci.style_set_fore(SCE_P_WORD, p.keyword);
    sci.style_set_bold(SCE_P_WORD, true);
    sci.style_set_fore(SCE_P_CLASSNAME, p.keyword2);
    sci.style_set_fore(SCE_P_DEFNAME, p.keyword2);
    sci.style_set_fore(SCE_P_OPERATOR, p.operator);
    sci.style_set_fore(SCE_P_DECORATOR, p.preproc);
}

fn apply_rust_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_RUST_COMMENTBLOCK, p.comment);
    sci.style_set_fore(SCE_RUST_COMMENTLINE, p.comment);
    sci.style_set_fore(SCE_RUST_COMMENTBLOCKDOC, p.comment);
    sci.style_set_fore(SCE_RUST_COMMENTLINEDOC, p.comment);
    sci.style_set_fore(SCE_RUST_NUMBER, p.number);
    sci.style_set_fore(SCE_RUST_WORD, p.keyword);
    sci.style_set_bold(SCE_RUST_WORD, true);
    sci.style_set_fore(SCE_RUST_WORD2, p.keyword2);
    sci.style_set_fore(SCE_RUST_STRING, p.string);
    sci.style_set_fore(SCE_RUST_STRINGR, p.string);
    sci.style_set_fore(SCE_RUST_CHARACTER, p.string);
    sci.style_set_fore(SCE_RUST_OPERATOR, p.operator);
    sci.style_set_fore(SCE_RUST_LIFETIME, p.label);
    sci.style_set_fore(SCE_RUST_MACRO, p.preproc);
}

fn apply_html_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_H_TAG, p.tag);
    sci.style_set_bold(SCE_H_TAG, true);
    sci.style_set_fore(SCE_H_TAGEND, p.tag);
    sci.style_set_bold(SCE_H_TAGEND, true);
    sci.style_set_fore(SCE_H_ATTRIBUTE, p.attr);
    sci.style_set_fore(SCE_H_DOUBLESTRING, p.string);
    sci.style_set_fore(SCE_H_SINGLESTRING, p.string);
    sci.style_set_fore(SCE_H_COMMENT, p.comment);
}

fn apply_css_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_CSS_TAG, p.tag);
    sci.style_set_fore(SCE_CSS_CLASS, p.keyword);
    sci.style_set_bold(SCE_CSS_CLASS, true);
    sci.style_set_fore(SCE_CSS_PSEUDOCLASS, p.keyword2);
    sci.style_set_fore(SCE_CSS_OPERATOR, p.operator);
    sci.style_set_fore(SCE_CSS_IDENTIFIER, p.keyword);
    sci.style_set_fore(SCE_CSS_VALUE, p.string);
    sci.style_set_fore(SCE_CSS_COMMENT, p.comment);
    sci.style_set_fore(SCE_CSS_ID, p.keyword2);
    sci.style_set_bold(SCE_CSS_ID, true);
    sci.style_set_fore(SCE_CSS_IMPORTANT, p.preproc);
    sci.style_set_bold(SCE_CSS_IMPORTANT, true);
    sci.style_set_fore(SCE_CSS_SINGLESTRING, p.string);
    sci.style_set_fore(SCE_CSS_DOUBLESTRING, p.string);
}

fn apply_json_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_JSON_NUMBER, p.number);
    sci.style_set_fore(SCE_JSON_STRING, p.string);
    sci.style_set_fore(SCE_JSON_PROPERTYNAME, p.keyword);
    sci.style_set_bold(SCE_JSON_PROPERTYNAME, true);
    sci.style_set_fore(SCE_JSON_OPERATOR, p.operator);
    sci.style_set_fore(SCE_JSON_KEYWORD, p.keyword2);
}

fn apply_sql_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_SQL_COMMENT, p.comment);
    sci.style_set_fore(SCE_SQL_COMMENTLINE, p.comment);
    sci.style_set_fore(SCE_SQL_COMMENTDOC, p.comment);
    sci.style_set_fore(SCE_SQL_NUMBER, p.number);
    sci.style_set_fore(SCE_SQL_WORD, p.keyword);
    sci.style_set_bold(SCE_SQL_WORD, true);
    sci.style_set_fore(SCE_SQL_STRING, p.string);
    sci.style_set_fore(SCE_SQL_CHARACTER, p.string);
    sci.style_set_fore(SCE_SQL_OPERATOR, p.operator);
}

fn apply_toml_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_TOML_COMMENT, p.comment);
    sci.style_set_fore(SCE_TOML_SECTIONTITLE, p.section);
    sci.style_set_bold(SCE_TOML_SECTIONTITLE, true);
    sci.style_set_fore(SCE_TOML_KEY, p.key);
    sci.style_set_fore(SCE_TOML_NUMBER, p.number);
    sci.style_set_fore(SCE_TOML_STRING, p.string);
    sci.style_set_fore(SCE_TOML_STRINGMULTILINE, p.string);
    sci.style_set_fore(SCE_TOML_BOOL, p.keyword);
    sci.style_set_bold(SCE_TOML_BOOL, true);
}

fn apply_ini_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_PROPS_COMMENT, p.comment);
    sci.style_set_fore(SCE_PROPS_SECTION, p.section);
    sci.style_set_bold(SCE_PROPS_SECTION, true);
    sci.style_set_fore(SCE_PROPS_KEY, p.key);
}

fn apply_batch_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_BAT_COMMENT, p.comment);
    sci.style_set_fore(SCE_BAT_WORD, p.keyword);
    sci.style_set_bold(SCE_BAT_WORD, true);
    sci.style_set_fore(SCE_BAT_LABEL, p.label);
    sci.style_set_fore(SCE_BAT_COMMAND, p.keyword2);
    sci.style_set_fore(SCE_BAT_OPERATOR, p.operator);
}

fn apply_makefile_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_MAKE_COMMENT, p.comment);
    sci.style_set_fore(SCE_MAKE_PREPROCESSOR, p.preproc);
    sci.style_set_fore(SCE_MAKE_TARGET, p.keyword);
    sci.style_set_bold(SCE_MAKE_TARGET, true);
    sci.style_set_fore(SCE_MAKE_OPERATOR, p.operator);
}

fn apply_diff_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_DIFF_COMMENT, p.comment);
    sci.style_set_fore(SCE_DIFF_COMMAND, p.preproc);
    sci.style_set_fore(SCE_DIFF_HEADER, p.diff_hdr);
    sci.style_set_bold(SCE_DIFF_HEADER, true);
    sci.style_set_fore(SCE_DIFF_POSITION, p.keyword2);
    sci.style_set_fore(SCE_DIFF_DELETED, p.diff_del);
    sci.style_set_fore(SCE_DIFF_ADDED, p.diff_add);
}

fn apply_shell_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_SH_COMMENTLINE, p.comment);
    sci.style_set_fore(SCE_SH_NUMBER, p.number);
    sci.style_set_fore(SCE_SH_WORD, p.keyword);
    sci.style_set_bold(SCE_SH_WORD, true);
    sci.style_set_fore(SCE_SH_STRING, p.string);
    sci.style_set_fore(SCE_SH_CHARACTER, p.string);
    sci.style_set_fore(SCE_SH_OPERATOR, p.operator);
    sci.style_set_fore(SCE_SH_SCALAR, p.keyword2);
}

fn apply_markdown_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_MARKDOWN_STRONG1, p.fg);
    sci.style_set_bold(SCE_MARKDOWN_STRONG1, true);
    sci.style_set_fore(SCE_MARKDOWN_STRONG2, p.fg);
    sci.style_set_bold(SCE_MARKDOWN_STRONG2, true);
    sci.style_set_fore(SCE_MARKDOWN_EM1, p.fg);
    sci.style_set_fore(SCE_MARKDOWN_EM2, p.fg);
    sci.style_set_fore(SCE_MARKDOWN_HEADER1, p.md_header);
    sci.style_set_bold(SCE_MARKDOWN_HEADER1, true);
    sci.style_set_fore(SCE_MARKDOWN_HEADER2, p.md_header);
    sci.style_set_bold(SCE_MARKDOWN_HEADER2, true);
    sci.style_set_fore(SCE_MARKDOWN_HEADER3, p.md_header);
    sci.style_set_bold(SCE_MARKDOWN_HEADER3, true);
    sci.style_set_fore(SCE_MARKDOWN_HEADER4, p.md_header);
    sci.style_set_fore(SCE_MARKDOWN_HEADER5, p.md_header);
    sci.style_set_fore(SCE_MARKDOWN_HEADER6, p.md_header);
    sci.style_set_fore(SCE_MARKDOWN_ULIST_ITEM, p.keyword2);
    sci.style_set_fore(SCE_MARKDOWN_OLIST_ITEM, p.keyword2);
    sci.style_set_fore(SCE_MARKDOWN_BLOCKQUOTE, p.comment);
    sci.style_set_fore(SCE_MARKDOWN_STRIKEOUT, p.label);
    sci.style_set_fore(SCE_MARKDOWN_HRULE, p.keyword2);
    sci.style_set_fore(SCE_MARKDOWN_LINK, p.keyword);
    sci.style_set_fore(SCE_MARKDOWN_CODE, p.md_code);
    sci.style_set_fore(SCE_MARKDOWN_CODE2, p.md_code);
    sci.style_set_fore(SCE_MARKDOWN_CODEBK, p.md_code);
}

fn apply_yaml_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_YAML_COMMENT, p.comment);
    sci.style_set_fore(SCE_YAML_IDENTIFIER, p.yaml_key);
    sci.style_set_bold(SCE_YAML_IDENTIFIER, true);
    sci.style_set_fore(SCE_YAML_KEYWORD, p.keyword);
    sci.style_set_bold(SCE_YAML_KEYWORD, true);
    sci.style_set_fore(SCE_YAML_NUMBER, p.number);
    sci.style_set_fore(SCE_YAML_DOCUMENT, p.keyword2);
    sci.style_set_fore(SCE_YAML_TEXT, p.string);
    sci.style_set_fore(SCE_YAML_OPERATOR, p.operator);
}

fn apply_powershell_theme(sci: &ScintillaView, p: &Palette) {
    sci.style_set_fore(SCE_POWERSHELL_COMMENT, p.comment);
    sci.style_set_fore(SCE_POWERSHELL_COMMENTSTREAM, p.comment);
    sci.style_set_fore(SCE_POWERSHELL_STRING, p.string);
    sci.style_set_fore(SCE_POWERSHELL_CHARACTER, p.string);
    sci.style_set_fore(SCE_POWERSHELL_HERE_STRING, p.string);
    sci.style_set_fore(SCE_POWERSHELL_HERE_CHARACTER, p.string);
    sci.style_set_fore(SCE_POWERSHELL_NUMBER, p.number);
    sci.style_set_fore(SCE_POWERSHELL_VARIABLE, p.keyword2);
    sci.style_set_fore(SCE_POWERSHELL_OPERATOR, p.operator);
    sci.style_set_fore(SCE_POWERSHELL_KEYWORD, p.keyword);
    sci.style_set_bold(SCE_POWERSHELL_KEYWORD, true);
    sci.style_set_fore(SCE_POWERSHELL_CMDLET, p.keyword2);
    sci.style_set_fore(SCE_POWERSHELL_FUNCTION, p.preproc);
}
