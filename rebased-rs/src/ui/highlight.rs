//! 代码编辑区的轻量语法高亮。
//!
//! 对齐 IntelliJ「编辑器着色」的观感但只做预览级：按扩展名识别语言，
//! 用逐行状态机切分 token（关键字 / 类型 / 函数 / 字符串 / 数字 / 注释 /
//! 注解），再由 [`crate::ui::editor_view`] 映射到 Darcula 色板。跨行状态
//! 只有块注释一种——足以覆盖常见文件，又避免引入完整 parser 的体积成本。
//!
//! 本模块是纯文本处理，不依赖 gpui：token 只带语义类别，颜色由渲染层决定，
//! 因此可以脱离窗口环境直接单测。

/// 按文件扩展名识别的语言集合。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lang {
    /// 纯文本：不切分，整体按正文色渲染。
    Plain,
    Rust,
    CFamily,
    Java,
    Go,
    Python,
    JavaScript,
    TypeScript,
    Json,
    Markdown,
    Toml,
    Yaml,
    Shell,
}

/// token 的语义类别（渲染层按类别取 Darcula 色）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Plain,
    Keyword,
    Type,
    Function,
    Str,
    Number,
    Comment,
    /// 文档注释（Rust 的 `///` `//!`）。
    Doc,
    /// 注解 / 属性：`#[derive(..)]`、`@Override`、预处理指令、配置键。
    Attr,
}

/// 一段连续同色的文本。
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) text: String,
    pub(crate) kind: TokenKind,
}

/// 跨行扫描状态：目前只有未闭合的块注释需要延续到下一行。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct HighlightState {
    in_block_comment: bool,
}

/// 由路径扩展名识别语言（无扩展名 / 未知扩展名按纯文本）。
pub(crate) fn lang_of(path: &str) -> Lang {
    let ext = path.rsplit('.').next().unwrap_or("");
    match ext {
        "rs" => Lang::Rust,
        "c" | "h" | "cc" | "cpp" | "cxx" | "hpp" | "hh" | "hxx" => Lang::CFamily,
        "java" | "kt" | "kts" | "scala" => Lang::Java,
        "go" => Lang::Go,
        "py" | "pyi" => Lang::Python,
        "js" | "jsx" | "mjs" | "cjs" => Lang::JavaScript,
        "ts" | "tsx" | "mts" | "cts" => Lang::TypeScript,
        "json" | "jsonc" => Lang::Json,
        "md" | "markdown" => Lang::Markdown,
        "toml" => Lang::Toml,
        "yml" | "yaml" => Lang::Yaml,
        "sh" | "bash" | "zsh" => Lang::Shell,
        _ => Lang::Plain,
    }
}

/// 对单行做高亮切分；`state` 在多行内容上按顺序传递以延续块注释。
pub(crate) fn highlight_line(line: &str, lang: Lang, state: &mut HighlightState) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut rest = line;
    // 延续上一行未闭合的块注释，直到本行出现 `*/`。
    if state.in_block_comment {
        match rest.find("*/") {
            Some(end) => {
                push(&mut spans, &rest[..end + 2], TokenKind::Comment);
                rest = &rest[end + 2..];
                state.in_block_comment = false;
            }
            None => {
                push(&mut spans, rest, TokenKind::Comment);
                return spans;
            }
        }
    }
    match lang {
        Lang::Markdown => markdown_line(rest, &mut spans),
        Lang::Plain => push(&mut spans, rest, TokenKind::Plain),
        _ => scan(rest, lang, &mut spans, state),
    }
    spans
}

/// 追加片段；相邻同类别片段合并，控制 span 数量。
fn push(spans: &mut Vec<Span>, text: &str, kind: TokenKind) {
    if text.is_empty() {
        return;
    }
    match spans.last_mut() {
        Some(last) if last.kind == kind => last.text.push_str(text),
        _ => spans.push(Span {
            text: text.to_string(),
            kind,
        }),
    }
}

/// Markdown：标题行 / 围栏行整行着色，行内 `` `code` `` 按字符串色。
fn markdown_line(line: &str, spans: &mut Vec<Span>) {
    let trimmed = line.trim_start();
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) && trimmed[hashes..].starts_with(' ') {
        push(spans, line, TokenKind::Function);
        return;
    }
    if trimmed.starts_with("```") {
        push(spans, line, TokenKind::Attr);
        return;
    }
    // 行内 `code`（含反引号本身）按字符串色，其余为正文。
    let mut rest = line;
    while let Some(start) = rest.find('`') {
        push(spans, &rest[..start], TokenKind::Plain);
        match rest[start + 1..].find('`') {
            Some(end) => {
                push(spans, &rest[start..start + end + 2], TokenKind::Str);
                rest = &rest[start + end + 2..];
            }
            None => {
                push(spans, &rest[start..], TokenKind::Plain);
                return;
            }
        }
    }
    push(spans, rest, TokenKind::Plain);
}

/// 通用逐字符扫描：字符串 / 数字 / 标识符 / 注释 / 注解。
fn scan(line: &str, lang: Lang, spans: &mut Vec<Span>, state: &mut HighlightState) {
    let line_comments = matches!(
        lang,
        Lang::Rust | Lang::CFamily | Lang::Java | Lang::Go | Lang::JavaScript | Lang::TypeScript
    );
    let block_comments = line_comments;
    let hash_comments = matches!(lang, Lang::Python | Lang::Shell | Lang::Toml | Lang::Yaml);
    let annotations = matches!(
        lang,
        Lang::Java | Lang::Python | Lang::JavaScript | Lang::TypeScript
    );
    let key_aware = matches!(lang, Lang::Json | Lang::Yaml | Lang::Toml);

    let mut index = 0;
    while index < line.len() {
        let c = line[index..].chars().next().expect("扫描位置必然有字符");

        if c == ' ' || c == '\t' {
            let end = line[index..]
                .find(|ch: char| ch != ' ' && ch != '\t')
                .map_or(line.len(), |off| index + off);
            push(spans, &line[index..end], TokenKind::Plain);
            index = end;
            continue;
        }
        // 行注释（`//`）与 hash 注释（`#`）都吃掉整行剩余部分。
        if line_comments && line[index..].starts_with("//") {
            let doc = line[index..].starts_with("///") || line[index..].starts_with("//!");
            push(
                spans,
                &line[index..],
                if doc {
                    TokenKind::Doc
                } else {
                    TokenKind::Comment
                },
            );
            return;
        }
        if hash_comments && c == '#' {
            push(spans, &line[index..], TokenKind::Comment);
            return;
        }
        if block_comments && line[index..].starts_with("/*") {
            match line[index + 2..].find("*/") {
                Some(off) => {
                    let end = index + 2 + off + 2;
                    push(spans, &line[index..end], TokenKind::Comment);
                    index = end;
                }
                None => {
                    push(spans, &line[index..], TokenKind::Comment);
                    state.in_block_comment = true;
                    return;
                }
            }
            continue;
        }
        // Rust 属性 `#[..]` / C 预处理指令。
        if c == '#' {
            if lang == Lang::Rust
                && (line[index..].starts_with("#[") || line[index..].starts_with("#!["))
            {
                let end = line[index..]
                    .find(']')
                    .map_or(line.len(), |off| index + off + 1);
                push(spans, &line[index..end], TokenKind::Attr);
                index = end;
                continue;
            }
            if lang == Lang::CFamily {
                push(spans, &line[index..], TokenKind::Attr);
                return;
            }
            push(spans, &line[index..index + c.len_utf8()], TokenKind::Plain);
            index += c.len_utf8();
            continue;
        }
        // Java / Python / TS 风格注解与装饰器。
        if annotations
            && c == '@'
            && line[index + 1..].starts_with(|ch: char| ch.is_ascii_alphabetic() || ch == '_')
        {
            let end = line[index + 1..]
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .map_or(line.len(), |off| index + 1 + off);
            push(spans, &line[index..end], TokenKind::Attr);
            index = end;
            continue;
        }
        // 字符串：单双引号通用；JS/TS 的反引号模板串同色。
        if c == '"'
            || c == '\''
            || (c == '`' && matches!(lang, Lang::JavaScript | Lang::TypeScript))
        {
            // Rust 的 `'` 有歧义：优先按「字符字面量 vs 生命周期」区分。
            if lang == Lang::Rust && c == '\'' {
                index = scan_rust_quote(line, index, spans);
                continue;
            }
            let (end, closed) = scan_string_end(line, index, c);
            let end = end.min(line.len());
            // JSON/YAML/TOML：出现在键位置的字符串按注解色（键值分明）。
            let kind = if closed && key_aware && is_key_sep(line, end, lang) {
                TokenKind::Attr
            } else {
                TokenKind::Str
            };
            push(spans, &line[index..end], kind);
            index = end;
            continue;
        }
        if c.is_ascii_digit() {
            let end = line[index..]
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '.' || ch == '_'))
                .map_or(line.len(), |off| index + off);
            // `0..10` 这类区间：结尾连续的 `.` 不属于数字。
            let mut end = end;
            while end > index + 1 && line.as_bytes()[end - 1] == b'.' {
                end -= 1;
            }
            push(spans, &line[index..end], TokenKind::Number);
            index = end;
            continue;
        }
        if c.is_alphabetic() || c == '_' {
            let end = line[index..]
                .find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))
                .map_or(line.len(), |off| index + off);
            let word = &line[index..end];
            let kind = classify(word, line, end, lang, key_aware);
            push(spans, word, kind);
            index = end;
            continue;
        }
        push(spans, &line[index..index + c.len_utf8()], TokenKind::Plain);
        index += c.len_utf8();
    }
}

/// 从 `start` 处的引号开始扫描字符串结尾，返回（结尾下标, 是否闭合）。
/// 支持反斜杠转义；行内未闭合的字符串按到行尾处理。
fn scan_string_end(line: &str, start: usize, quote: char) -> (usize, bool) {
    let mut end = start + quote.len_utf8();
    while end < line.len() {
        let ch = line[end..].chars().next().expect("扫描位置必然有字符");
        if ch == '\\' {
            // 跳过转义符与其后的一个字符。
            end += ch.len_utf8();
            end += line[end..].chars().next().map_or(1, |n| n.len_utf8());
            continue;
        }
        end += ch.len_utf8();
        if ch == quote {
            return (end, true);
        }
    }
    (end, false)
}

/// Rust 的 `'` 有歧义：`'x'` 是字符字面量，`'a` 是生命周期。
/// 行内找不到配对引号时按生命周期处理（只吃一个 `'`，按正文色）。
fn scan_rust_quote(line: &str, start: usize, spans: &mut Vec<Span>) -> usize {
    let (end, closed) = scan_string_end(line, start, '\'');
    if closed && line[start + 1..end - 1].chars().count() <= 2 {
        push(spans, &line[start..end.min(line.len())], TokenKind::Str);
        return end.min(line.len());
    }
    push(spans, "'", TokenKind::Plain);
    start + 1
}

/// 字符串 / 标识符之后是否紧跟键分隔符（JSON `:`、TOML `=`、YAML `:`）。
fn is_key_sep(line: &str, end: usize, lang: Lang) -> bool {
    let rest = line[end..].trim_start();
    match lang {
        Lang::Json => rest.starts_with(':'),
        Lang::Toml => rest.starts_with('='),
        Lang::Yaml => rest.starts_with(": ") || rest == ":",
        _ => false,
    }
}

/// 标识符定类：关键字 > 内建类型 > 键名 > 函数调用 > 大写开头类型 > 正文。
fn classify(word: &str, line: &str, end: usize, lang: Lang, key_aware: bool) -> TokenKind {
    if keywords(lang).contains(&word) {
        return TokenKind::Keyword;
    }
    if types(lang).contains(&word) {
        return TokenKind::Type;
    }
    if key_aware && is_key_sep(line, end, lang) {
        return TokenKind::Attr;
    }
    if line[end..].starts_with('(') {
        return TokenKind::Function;
    }
    let upper_case = matches!(
        lang,
        Lang::Rust
            | Lang::CFamily
            | Lang::Java
            | Lang::Go
            | Lang::Python
            | Lang::JavaScript
            | Lang::TypeScript
    );
    if upper_case && word.chars().next().is_some_and(char::is_uppercase) {
        return TokenKind::Type;
    }
    TokenKind::Plain
}

/// 各语言的关键字表（按需覆盖常见文件，不求完备）。
fn keywords(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Rust => &[
            "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
            "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod",
            "move", "mut", "pub", "ref", "return", "self", "static", "struct", "super", "trait",
            "true", "type", "unsafe", "use", "where", "while", "yield",
        ],
        Lang::CFamily => &[
            "alignas", "alignof", "auto", "break", "case", "catch", "class", "const", "constexpr",
            "continue", "default", "delete", "do", "else", "enum", "explicit", "export", "extern",
            "false", "final", "for", "friend", "goto", "if", "inline", "mutable", "namespace",
            "new", "noexcept", "nullptr", "operator", "override", "private", "protected",
            "public", "register", "return", "sizeof", "static", "static_assert", "static_cast",
            "struct", "switch", "template", "this", "throw", "true", "try", "typedef", "typename",
            "union", "using", "virtual", "volatile", "while",
        ],
        Lang::Java => &[
            "abstract", "assert", "break", "case", "catch", "class", "const", "continue",
            "default", "do", "else", "enum", "extends", "false", "final", "finally", "float",
            "for", "goto", "if", "implements", "import", "instanceof", "interface", "native",
            "new", "null", "package", "permits", "private", "protected", "public", "record",
            "return", "sealed", "static", "strictfp", "super", "switch", "synchronized", "this",
            "throw", "throws", "transient", "true", "try", "var", "void", "volatile", "while",
            "yield",
        ],
        Lang::Go => &[
            "break", "case", "chan", "const", "continue", "default", "defer", "else",
            "fallthrough", "false", "for", "func", "go", "goto", "if", "import", "interface",
            "map", "nil", "package", "range", "return", "select", "struct", "switch", "true",
            "type", "var",
        ],
        Lang::Python => &[
            "False", "None", "True", "and", "as", "assert", "async", "await", "break", "case",
            "class", "continue", "def", "del", "elif", "else", "except", "finally", "for",
            "from", "global", "if", "import", "in", "is", "lambda", "match", "nonlocal", "not",
            "or", "pass", "raise", "return", "try", "while", "with", "yield",
        ],
        Lang::JavaScript | Lang::TypeScript => &[
            "abstract", "any", "as", "async", "await", "bigint", "boolean", "break", "case",
            "catch", "class", "const", "continue", "debugger", "declare", "default", "delete",
            "do", "else", "enum", "export", "extends", "false", "finally", "for", "from",
            "function", "get", "if", "implements", "import", "in", "infer", "instanceof",
            "interface", "is", "keyof", "let", "namespace", "never", "new", "null", "number",
            "object", "of", "private", "protected", "public", "readonly", "return", "satisfies",
            "set", "static", "string", "super", "switch", "symbol", "this", "throw", "true",
            "try", "type", "typeof", "undefined", "unknown", "var", "void", "while", "with",
            "yield",
        ],
        Lang::Json => &["false", "null", "true"],
        Lang::Toml | Lang::Yaml => &["false", "null", "true"],
        Lang::Shell => &[
            "break", "case", "continue", "do", "done", "echo", "elif", "else", "esac", "exit",
            "export", "fi", "for", "function", "if", "in", "local", "read", "return", "select",
            "set", "source", "then", "time", "until", "unset", "while",
        ],
        Lang::Markdown | Lang::Plain => &[],
    }
}

/// 各语言的内建类型表。
fn types(lang: Lang) -> &'static [&'static str] {
    match lang {
        Lang::Rust => &[
            "Err", "Box", "Option", "Ok", "Result", "Self", "Some", "String", "Vec", "bool",
            "char", "f32", "f64", "i128", "i16", "i32", "i64", "i8", "isize", "str", "u128",
            "u16", "u32", "u64", "u8", "usize",
        ],
        Lang::CFamily => &[
            "bool", "char", "double", "float", "int", "long", "short", "signed", "size_t",
            "ssize_t", "unsigned", "void",
        ],
        Lang::Java => &[
            "Boolean", "Byte", "Character", "Double", "Float", "HashMap", "HashSet", "Integer",
            "List", "Long", "Map", "Object", "Set", "Short", "String", "boolean", "byte", "char",
            "double", "float", "int", "long", "short",
        ],
        Lang::Go => &[
            "any", "bool", "byte", "error", "float32", "float64", "int", "int16", "int32",
            "int64", "int8", "rune", "string", "uint", "uint16", "uint32", "uint64", "uint8",
            "uintptr",
        ],
        Lang::Python => &[
            "bool", "bytes", "dict", "float", "frozenset", "int", "list", "object", "set", "str",
            "tuple", "type",
        ],
        Lang::JavaScript | Lang::TypeScript => &[
            "Array", "BigInt", "Boolean", "Map", "Number", "Object", "Promise", "RegExp", "Set",
            "String", "WeakMap", "WeakSet",
        ],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(spans: &[Span]) -> Vec<TokenKind> {
        spans.iter().map(|span| span.kind).collect()
    }

    fn texts(spans: &[Span]) -> Vec<String> {
        spans.iter().map(|span| span.text.clone()).collect()
    }

    #[test]
    fn unknown_extension_is_plain() {
        assert_eq!(lang_of("README"), Lang::Plain);
        assert_eq!(lang_of("src/main.rs"), Lang::Rust);
        assert_eq!(lang_of("Cargo.toml"), Lang::Toml);
        assert_eq!(lang_of("data.yml"), Lang::Yaml);
    }

    #[test]
    fn rust_line_is_tokenized() {
        let mut state = HighlightState::default();
        let spans = highlight_line(
            "pub fn main() { let x: u32 = 42; } // tail",
            Lang::Rust,
            &mut state,
        );
        // 相邻同类别片段会被合并（例如 `() { ` 是一个 Plain 片段）。
        assert_eq!(
            texts(&spans),
            vec![
                "pub",
                " ",
                "fn",
                " ",
                "main",
                "() { ",
                "let",
                " x: ",
                "u32",
                " = ",
                "42",
                "; } ",
                "// tail",
            ]
        );
        assert_eq!(
            kinds(&spans),
            vec![
                TokenKind::Keyword,
                TokenKind::Plain,
                TokenKind::Keyword,
                TokenKind::Plain,
                TokenKind::Function,
                TokenKind::Plain,
                TokenKind::Keyword,
                TokenKind::Plain,
                TokenKind::Type,
                TokenKind::Plain,
                TokenKind::Number,
                TokenKind::Plain,
                TokenKind::Comment,
            ]
        );
    }

    #[test]
    fn rust_doc_comment_is_doc() {
        let mut state = HighlightState::default();
        let spans = highlight_line("/// 说明", Lang::Rust, &mut state);
        assert_eq!(kinds(&spans), vec![TokenKind::Doc]);
        let spans = highlight_line("//! 内部", Lang::Rust, &mut state);
        assert_eq!(kinds(&spans), vec![TokenKind::Doc]);
    }

    #[test]
    fn rust_attribute_is_attr() {
        let mut state = HighlightState::default();
        let spans = highlight_line("#[derive(Debug)]", Lang::Rust, &mut state);
        assert_eq!(kinds(&spans), vec![TokenKind::Attr]);
        assert_eq!(texts(&spans), vec!["#[derive(Debug)]"]);
    }

    #[test]
    fn rust_lifetime_is_not_string() {
        let mut state = HighlightState::default();
        let spans = highlight_line("fn f<'a>(x: &'a str) -> &'a str { 'x' }", Lang::Rust, &mut state);
        assert!(spans.iter().any(|span| span.kind == TokenKind::Str && span.text == "'x'"));
        // 生命周期 `'a` 不该被当字符串。
        assert!(!spans.iter().any(|span| span.text.contains("'a") && span.kind == TokenKind::Str));
    }

    #[test]
    fn block_comment_spans_lines() {
        let mut state = HighlightState::default();
        let first = highlight_line("let a = 1; /* start", Lang::Rust, &mut state);
        assert!(first.iter().any(|span| span.kind == TokenKind::Comment));
        assert!(state.in_block_comment);
        let second = highlight_line("middle */ let b = 2;", Lang::Rust, &mut state);
        assert_eq!(second.first().expect("应有片段").kind, TokenKind::Comment);
        assert!(second
            .iter()
            .any(|span| span.kind == TokenKind::Keyword && span.text == "let"));
        assert!(!state.in_block_comment);
    }

    #[test]
    fn python_string_and_comment() {
        let mut state = HighlightState::default();
        let spans = highlight_line("s = \"hi\"  # note", Lang::Python, &mut state);
        assert!(spans.contains(&Span { text: "\"hi\"".into(), kind: TokenKind::Str }));
        assert_eq!(spans.last().expect("应有片段").kind, TokenKind::Comment);
    }

    #[test]
    fn json_key_is_attr_value_is_string() {
        let mut state = HighlightState::default();
        let spans = highlight_line("{\"name\": \"demo\", \"n\": 3, ok: true}", Lang::Json, &mut state);
        assert!(spans.contains(&Span { text: "\"name\"".into(), kind: TokenKind::Attr }));
        assert!(spans.contains(&Span { text: "\"demo\"".into(), kind: TokenKind::Str }));
        assert!(spans.contains(&Span { text: "3".into(), kind: TokenKind::Number }));
        assert!(spans.contains(&Span { text: "true".into(), kind: TokenKind::Keyword }));
    }

    #[test]
    fn toml_key_is_attr() {
        let mut state = HighlightState::default();
        let spans = highlight_line("name = \"rebased-rs\" # cfg", Lang::Toml, &mut state);
        assert!(spans.contains(&Span { text: "name".into(), kind: TokenKind::Attr }));
        assert!(spans.contains(&Span { text: "\"rebased-rs\"".into(), kind: TokenKind::Str }));
        assert_eq!(spans.last().expect("应有片段").kind, TokenKind::Comment);
    }

    #[test]
    fn markdown_heading_and_code() {
        let mut state = HighlightState::default();
        let spans = highlight_line("## 标题", Lang::Markdown, &mut state);
        assert_eq!(kinds(&spans), vec![TokenKind::Function]);
        let spans = highlight_line("use `gpui` now", Lang::Markdown, &mut state);
        assert!(spans.contains(&Span { text: "`gpui`".into(), kind: TokenKind::Str }));
    }

    #[test]
    fn shell_and_preprocessor() {
        let mut state = HighlightState::default();
        let spans = highlight_line("if [ -f x ]; then", Lang::Shell, &mut state);
        assert_eq!(spans[0].kind, TokenKind::Keyword);
        let spans = highlight_line("#include <stdio.h>", Lang::CFamily, &mut state);
        assert_eq!(kinds(&spans), vec![TokenKind::Attr]);
    }

    #[test]
    fn plain_language_keeps_text_intact() {
        let mut state = HighlightState::default();
        let spans = highlight_line("just text 中文字符", Lang::Plain, &mut state);
        assert_eq!(texts(&spans), vec!["just text 中文字符"]);
        assert_eq!(kinds(&spans), vec![TokenKind::Plain]);
    }

    #[test]
    fn spans_concatenate_to_original() {
        let mut state = HighlightState::default();
        let line = "fn f() -> Option<u32> { x.0 + 1..10 } // ✓ 中文";
        let spans = highlight_line(line, Lang::Rust, &mut state);
        let joined: String = spans.iter().map(|span| span.text.as_str()).collect();
        assert_eq!(joined, line);
    }
}
