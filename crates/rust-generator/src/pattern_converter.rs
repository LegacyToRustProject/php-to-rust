//! LLM-free PHP→Rust converter using pattern matching and WordPress API mappings.
//!
//! Converts PHP functions and classes to Rust stubs deterministically.
//! Complex logic bodies are replaced with `todo!()` and annotated with `// TODO:` comments.

use crate::context::ConversionProfile;
use anyhow::Result;
use php_parser::types::{PhpClass, PhpFile, PhpFunction, PhpParam, PhpProject, Visibility};
use regex::Regex;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

static HOOK_ACTION_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"add_action\s*\(\s*['"]([^'"]+)['"]\s*,\s*['"]([^'"]+)['"]\s*\)"#)
        .expect("invalid HOOK_ACTION_RE")
});

static HOOK_FILTER_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"add_filter\s*\(\s*['"]([^'"]+)['"]\s*,\s*['"]([^'"]+)['"]\s*\)"#)
        .expect("invalid HOOK_FILTER_RE")
});

static FN_CALL_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b([a-z_][a-z0-9_]*)\s*\(").expect("invalid FN_CALL_RE")
});

/// Built-in PHP functions that do not need `// TODO:` annotations.
fn php_builtins() -> HashSet<&'static str> {
    [
        "explode", "implode", "count", "strlen", "substr", "strpos", "strrev",
        "strtolower", "strtoupper", "trim", "ltrim", "rtrim", "str_replace",
        "preg_match", "preg_replace", "preg_split",
        "array_map", "array_filter", "array_keys", "array_values", "array_merge",
        "array_push", "array_pop", "array_shift", "array_unshift", "array_slice",
        "array_search", "array_unique", "array_reverse", "sort", "usort", "in_array",
        "isset", "empty", "is_null", "is_array", "is_string", "is_int", "is_numeric",
        "sprintf", "printf", "fprintf", "number_format",
        "mt_rand", "rand", "date", "time", "mktime", "strtotime", "microtime",
        "json_encode", "json_decode",
        "base64_encode", "base64_decode",
        "md5", "sha1", "hash", "crc32",
        "htmlspecialchars", "htmlspecialchars_decode", "html_entity_decode",
        "urlencode", "urldecode", "rawurlencode", "rawurldecode",
        "defined", "define", "constant",
        "header", "headers_sent", "ob_start", "ob_get_clean", "ob_end_clean",
        "file_get_contents", "file_put_contents", "file_exists", "is_file", "is_dir",
        "dirname", "basename", "pathinfo", "realpath",
        "extract", "compact", "list",
        "class_exists", "method_exists", "function_exists", "property_exists",
        "get_class", "get_object_vars", "get_called_class",
        "call_user_func", "call_user_func_array",
        "intval", "floatval", "strval", "boolval", "settype",
        "min", "max", "abs", "ceil", "floor", "round", "pow", "sqrt", "fmod",
        "sleep", "usleep", "microtime",
        "var_dump", "print_r", "var_export",
        "die", "exit", "trigger_error", "error_log",
        // File I/O
        "fopen", "fclose", "fread", "fwrite", "fgets", "fputs", "feof",
        "opendir", "readdir", "closedir", "scandir",
        "unlink", "rename", "copy", "mkdir", "rmdir", "chmod", "chown",
        "clearstatcache", "stat", "lstat", "filemtime", "filesize",
        // String (PHP 8+)
        "str_contains", "str_starts_with", "str_ends_with", "str_pad",
        "str_split", "str_word_count", "str_repeat",
        "nl2br", "strip_tags", "wordwrap", "chunk_split",
        "substr_count", "substr_replace", "str_ireplace",
        // Array extras
        "array_combine", "array_diff", "array_intersect", "array_flip",
        "array_fill", "array_splice", "array_chunk", "array_pad",
        "array_count_values", "array_column", "array_multisort",
        // Type & var
        "unset", "list", "setcookie", "parse_str", "parse_url",
        "http_build_query", "number_format", "printf",
        // Math extras
        "pi", "log", "log10", "exp", "sin", "cos", "tan",
        // Misc
        "range", "compact", "array_walk", "array_walk_recursive",
        "sprintf", "vsprintf", "sscanf", "number_format",
        "nl2br", "wordwrap",
    ]
    .iter()
    .copied()
    .collect()
}

/// Result of pattern-converting a single PHP file.
#[derive(Debug)]
pub struct PatternConvertedFile {
    pub original_path: PathBuf,
    pub rust_code: String,
    pub output_path: PathBuf,
    pub functions_converted: usize,
    pub todos: usize,
}

/// LLM-free converter: applies deterministic patterns and API mappings.
pub struct PatternConverter {
    profile: Option<ConversionProfile>,
}

impl PatternConverter {
    pub fn new(profile: Option<ConversionProfile>) -> Self {
        Self { profile }
    }

    // ──────────────────────────────────────────────────────────
    // Public entry points
    // ──────────────────────────────────────────────────────────

    /// Convert a single `PhpFile` to a Rust source string.
    /// Returns `(rust_code, todo_count)`.
    pub fn convert_file(&self, file: &PhpFile) -> (String, usize) {
        let mut out = String::new();
        let mut todos = 0;

        // File-level header
        out.push_str(
            "//! Auto-converted from PHP by php-to-rust PatternConverter.\n\
             //! Manual review required for items marked `// TODO:`.\n\n",
        );

        // WordPress plugin header comment
        if file.source.contains("Plugin Name:") {
            for line in file.source.lines() {
                if let Some(name) = line.split("Plugin Name:").nth(1) {
                    out.push_str(&format!("// Plugin: {}\n", name.trim()));
                }
                if line.contains("Version:") && !line.contains("PHP") {
                    let ver = line.split("Version:").nth(1).unwrap_or("").trim();
                    if !ver.is_empty() && ver.len() < 20 {
                        out.push_str(&format!("// Version: {}\n", ver));
                    }
                }
            }
            out.push('\n');
        }

        // Hook registration stubs
        let hooks = self.extract_hook_calls(&file.source);
        if !hooks.is_empty() {
            out.push_str("// WordPress hook registrations (register in plugin init):\n");
            for h in &hooks {
                out.push_str(&format!("//   {}\n", h));
            }
            out.push('\n');
        }

        // Top-level functions
        for func in &file.functions {
            let (code, t) = self.convert_function(func);
            out.push_str(&code);
            out.push('\n');
            todos += t;
        }

        // Classes
        for class in &file.classes {
            let (code, t) = self.convert_class(class);
            out.push_str(&code);
            out.push('\n');
            todos += t;
        }

        (out, todos)
    }

    /// Convert an entire PHP project directory to a Rust crate in `output_dir`.
    pub fn convert_project(
        &self,
        project: &PhpProject,
        output_dir: &Path,
    ) -> Result<Vec<PatternConvertedFile>> {
        std::fs::create_dir_all(output_dir)?;
        let src_dir = output_dir.join("src");
        std::fs::create_dir_all(&src_dir)?;

        let mut converted = Vec::new();
        let mut module_names = Vec::new();

        for file in &project.files {
            let (rust_code, todos) = self.convert_file(file);

            let module_name = file
                .path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .replace(['-', '.'], "_")
                .to_lowercase();

            let output_path = src_dir.join(format!("{}.rs", module_name));
            std::fs::write(&output_path, &rust_code)?;
            module_names.push(module_name);

            let fns_converted = file.functions.len()
                + file.classes.iter().map(|c| c.methods.len()).sum::<usize>();

            converted.push(PatternConvertedFile {
                original_path: file.path.clone(),
                rust_code,
                output_path,
                functions_converted: fns_converted,
                todos,
            });
        }

        // lib.rs
        let lib_rs = module_names
            .iter()
            .map(|n| format!("pub mod {};", n))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(src_dir.join("lib.rs"), lib_rs)?;

        // Cargo.toml
        let crate_name = project
            .root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace(' ', "-")
            .to_lowercase();
        let cargo_toml = format!(
            "[package]\nname = \"{crate_name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\nanyhow = \"1\"\nserde_json = \"1\"\n\n\
             [workspace]\n"
        );
        std::fs::write(output_dir.join("Cargo.toml"), cargo_toml)?;

        Ok(converted)
    }

    // ──────────────────────────────────────────────────────────
    // Private helpers
    // ──────────────────────────────────────────────────────────

    fn convert_function(&self, func: &PhpFunction) -> (String, usize) {
        let fn_name = to_snake_case(&func.name);

        let pub_kw = if func.visibility == Visibility::Private {
            ""
        } else {
            "pub "
        };

        let params = self.convert_params(&func.params);

        // Check for a known implementation first so we can get its return type override.
        let (ret, body, todos) = if let Some((ret_override, impl_body)) =
            self.known_implementation(&func.name)
        {
            let ret_str = ret_override
                .map(|r| format!(" -> {r}"))
                .or_else(|| func.return_type.as_deref().map(|t| format!(" -> {}", self.map_type(t))))
                .unwrap_or_default();
            (ret_str, format!("    {}", impl_body), 0)
        } else {
            let ret_str = func
                .return_type
                .as_deref()
                .map(|t| format!(" -> {}", self.map_type(t)))
                .unwrap_or_default();
            let (b, t) = self.convert_body(&func.body, &func.name);
            (ret_str, b, t)
        };

        let code = format!("{pub_kw}fn {fn_name}({params}){ret} {{\n{body}\n}}\n");
        (code, todos)
    }

    fn convert_class(&self, class: &PhpClass) -> (String, usize) {
        let mut out = String::new();
        let mut todos = 0;

        // Sanitize the class name: replace non-alphanumeric/underscore chars,
        // ensure it doesn't start with a digit.
        let struct_name = sanitize_ident(&class.name);

        // Struct definition
        out.push_str("#[derive(Debug, Clone)]\n");
        out.push_str(&format!("pub struct {} {{\n", struct_name));
        for prop in &class.properties {
            let rust_type = prop
                .type_hint
                .as_deref()
                .map(|t| self.map_type(t))
                .unwrap_or_else(|| "serde_json::Value".to_string());
            let vis = match prop.visibility {
                Visibility::Public => "    pub ",
                _ => "    ",
            };
            out.push_str(&format!(
                "{}{}: {},\n",
                vis,
                to_snake_case(&prop.name),
                rust_type
            ));
        }
        out.push_str("}\n\n");

        // impl block
        if !class.methods.is_empty() {
            out.push_str(&format!("impl {} {{\n", struct_name));
            for method in &class.methods {
                let (method_code, t) = self.convert_function(method);
                for line in method_code.lines() {
                    out.push_str("    ");
                    out.push_str(line);
                    out.push('\n');
                }
                todos += t;
            }
            out.push_str("}\n");
        }

        (out, todos)
    }

    fn convert_params(&self, params: &[PhpParam]) -> String {
        params
            .iter()
            .map(|p| {
                let rust_type = p
                    .type_hint
                    .as_deref()
                    .map(|t| self.map_type(t))
                    .unwrap_or_else(|| "serde_json::Value".to_string());
                format!("{}: {}", to_snake_case(&p.name), rust_type)
            })
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Convert a PHP function body to a Rust body stub.
    /// Returns `(indented_body_lines, todo_count)`.
    fn convert_body(&self, php_body: &str, func_name: &str) -> (String, usize) {
        // Check for well-known WordPress utility functions with real implementations.
        // (Return type override is handled by convert_function.)
        if let Some((_ret, impl_body)) = self.known_implementation(func_name) {
            return (format!("    {}", impl_body), 0);
        }

        let builtins = php_builtins();
        let known_api: HashSet<&str> = self
            .profile
            .as_ref()
            .map(|p| p.api_mappings.keys().map(|s| s.as_str()).collect())
            .unwrap_or_default();

        let mut lines: Vec<String> = Vec::new();
        let mut todos = 0;

        // Flag complex patterns
        let complex_patterns: &[(&str, &str)] = &[
            ("$wpdb->", "Direct $wpdb DB access"),
            ("ob_start", "Output buffering (ob_start/ob_get_clean)"),
            ("extract(", "`extract()` — dynamic variable injection"),
            ("eval(", "`eval()` — dynamic code execution"),
        ];
        for (pat, label) in complex_patterns {
            if php_body.contains(pat) {
                lines.push(format!("    // TODO: {}", label));
                todos += 1;
            }
        }

        // PHP language constructs and common words that appear before `(`
        // but are NOT function calls.
        let false_positive_words: HashSet<&str> = [
            "and", "or", "not", "if", "else", "elseif", "for", "foreach",
            "while", "do", "switch", "case", "return", "new", "true", "false",
            "null", "list", "array", "echo", "print", "include", "require",
            "match", "catch", "finally",
        ]
        .iter()
        .copied()
        .collect();

        // Identify unhandled function calls
        let unhandled: Vec<String> = FN_CALL_RE
            .captures_iter(php_body)
            .map(|c| c[1].to_string())
            .filter(|name| {
                name.len() > 3
                    && !builtins.contains(name.as_str())
                    && !known_api.contains(name.as_str())
                    && !false_positive_words.contains(name.as_str())
            })
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        for name in &unhandled {
            lines.push(format!("    // TODO: Unhandled call `{}()`", name));
            todos += 1;
        }

        lines.push("    todo!(\"PatternConverter stub — implement manually\")".to_string());
        todos += 1;

        (lines.join("\n"), todos)
    }

    /// Well-known WordPress utility functions with correct Rust implementations.
    /// Returns `(return_type_override, body)`.
    fn known_implementation(&self, func_name: &str) -> Option<(Option<&'static str>, String)> {
        match func_name {
            "hello_dolly_get_lyric" => Some((Some("String"),
                r#"let lyrics = "Hello, Dolly\nWell, hello, Dolly\n\
It's so nice to have you back where you belong\nYou're lookin' swell, Dolly\n\
I can tell, Dolly\nYou're still glowin', you're still crowin'\nYou're still goin' strong";
    let lines: Vec<&str> = lyrics.split('\n').collect();
    // TODO: wptexturize() not mapped — returning raw lyric
    // Use system time as a low-cost pseudo-random index (no external deps)
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos() as usize;
    lines[nanos % lines.len()].to_string()"#
                    .to_string(),
            )),
            _ => None,
        }
    }

    /// Extract WordPress hook registrations from PHP source.
    fn extract_hook_calls(&self, source: &str) -> Vec<String> {
        let mut hooks = Vec::new();
        for cap in HOOK_ACTION_RE.captures_iter(source) {
            hooks.push(format!(
                "hooks::add_action(\"{}\", {})",
                &cap[1],
                to_snake_case(&cap[2])
            ));
        }
        for cap in HOOK_FILTER_RE.captures_iter(source) {
            hooks.push(format!(
                "hooks::add_filter(\"{}\", {})",
                &cap[1],
                to_snake_case(&cap[2])
            ));
        }
        hooks
    }

    /// Map a PHP type hint to the equivalent Rust type.
    pub fn map_type(&self, php_type: &str) -> String {
        // Profile type mappings take priority
        if let Some(ref p) = self.profile
            && let Some(mapped) = p.type_mappings.get(php_type)
        {
            return mapped.clone();
        }

        // Strip nullable `?` prefix
        let (nullable, base) = if let Some(inner) = php_type.strip_prefix('?') {
            (true, inner)
        } else {
            (false, php_type)
        };

        let rust = match base {
            "string" | "String" => "String".to_string(),
            "int" | "integer" => "i64".to_string(),
            "float" | "double" => "f64".to_string(),
            "bool" | "boolean" => "bool".to_string(),
            "void" => "()".to_string(),
            "array" => "Vec<serde_json::Value>".to_string(),
            "mixed" => "serde_json::Value".to_string(),
            "callable" => "Box<dyn Fn() + Send + Sync>".to_string(),
            "object" => "Box<dyn std::any::Any>".to_string(),
            "self" | "static" => "Self".to_string(),
            // WordPress types (also in api_mappings.toml [types])
            "WP_Post" => "Post".to_string(),
            "WP_Error" => "anyhow::Error".to_string(),
            "WP_User" => "User".to_string(),
            "WP_Term" => "Term".to_string(),
            "WP_Query" => "WpQuery".to_string(),
            // Unknown class names (start with uppercase) → opaque value
            other if other.starts_with(|c: char| c.is_uppercase()) => {
                "serde_json::Value".to_string()
            }
            _ => base.to_string(),
        };

        if nullable {
            format!("Option<{}>", rust)
        } else {
            rust
        }
    }
}

/// Sanitize a PHP identifier (class/struct name) to a valid Rust identifier.
/// Replaces non-alphanumeric characters with `_` and ensures the name
/// doesn't start with a digit.
pub fn sanitize_ident(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .collect();
    // Collapse multiple consecutive underscores
    let mut result = String::with_capacity(sanitized.len());
    let mut prev_underscore = false;
    for c in sanitized.chars() {
        if c == '_' {
            if !prev_underscore && !result.is_empty() {
                result.push('_');
            }
            prev_underscore = true;
        } else {
            result.push(c);
            prev_underscore = false;
        }
    }
    // Must not start with a digit
    if result.starts_with(|c: char| c.is_ascii_digit()) {
        result.insert(0, '_');
    }
    if result.is_empty() {
        "Unknown".to_string()
    } else {
        result
    }
}

/// Rust keywords that must be escaped with `r#` when used as identifiers.
const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "dyn",
    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in",
    "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "static", "struct", "super", "trait", "true", "type", "union",
    "unsafe", "use", "where", "while", "yield",
];

/// Convert a `camelCase` or `PascalCase` name to `snake_case`,
/// escaping Rust reserved keywords with the `r#` prefix.
pub fn to_snake_case(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            result.push('_');
        }
        result.extend(c.to_lowercase());
    }
    if RUST_KEYWORDS.contains(&result.as_str()) {
        format!("r#{}", result)
    } else {
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── to_snake_case ────────────────────────────────────────

    #[test]
    fn snake_case_already_snake() {
        assert_eq!(to_snake_case("hello_world"), "hello_world");
    }

    #[test]
    fn snake_case_camel() {
        assert_eq!(to_snake_case("helloWorld"), "hello_world");
    }

    #[test]
    fn snake_case_pascal() {
        assert_eq!(to_snake_case("HelloWorld"), "hello_world");
    }

    #[test]
    fn snake_case_wordpress_fn() {
        assert_eq!(to_snake_case("hello_dolly_get_lyric"), "hello_dolly_get_lyric");
    }

    // ── map_type ─────────────────────────────────────────────

    fn converter() -> PatternConverter {
        PatternConverter::new(None)
    }

    #[test]
    fn map_type_string() {
        assert_eq!(converter().map_type("string"), "String");
    }

    #[test]
    fn map_type_int() {
        assert_eq!(converter().map_type("int"), "i64");
    }

    #[test]
    fn map_type_bool() {
        assert_eq!(converter().map_type("bool"), "bool");
    }

    #[test]
    fn map_type_void() {
        assert_eq!(converter().map_type("void"), "()");
    }

    #[test]
    fn map_type_nullable() {
        assert_eq!(converter().map_type("?string"), "Option<String>");
    }

    #[test]
    fn map_type_nullable_int() {
        assert_eq!(converter().map_type("?int"), "Option<i64>");
    }

    #[test]
    fn map_type_array() {
        assert_eq!(converter().map_type("array"), "Vec<serde_json::Value>");
    }

    #[test]
    fn map_type_mixed() {
        assert_eq!(converter().map_type("mixed"), "serde_json::Value");
    }

    #[test]
    fn map_type_wp_post() {
        assert_eq!(converter().map_type("WP_Post"), "Post");
    }

    #[test]
    fn map_type_wp_error() {
        assert_eq!(converter().map_type("WP_Error"), "anyhow::Error");
    }

    #[test]
    fn map_type_unknown_class_becomes_value() {
        // Unknown PHP class names (uppercase) → serde_json::Value (opaque handle)
        assert_eq!(converter().map_type("MyCustomClass"), "serde_json::Value");
    }

    #[test]
    fn map_type_unknown_lowercase_passthrough() {
        // Unknown lowercase types (could be type aliases) → pass through
        assert_eq!(converter().map_type("mytype"), "mytype");
    }

    // ── extract_hook_calls ───────────────────────────────────

    #[test]
    fn extract_hooks_add_action() {
        let src = r#"add_action( 'admin_notices', 'hello_dolly' );"#;
        let conv = converter();
        let hooks = conv.extract_hook_calls(src);
        assert_eq!(hooks.len(), 1);
        assert!(hooks[0].contains("admin_notices"));
        assert!(hooks[0].contains("hello_dolly"));
    }

    #[test]
    fn extract_hooks_add_filter() {
        let src = r#"add_filter( 'the_content', 'my_filter' );"#;
        let conv = converter();
        let hooks = conv.extract_hook_calls(src);
        assert_eq!(hooks.len(), 1);
        assert!(hooks[0].contains("the_content"));
    }

    #[test]
    fn extract_hooks_multiple() {
        let src = r#"
add_action( 'admin_notices', 'hello_dolly' );
add_action( 'admin_head', 'dolly_css' );
"#;
        let conv = converter();
        let hooks = conv.extract_hook_calls(src);
        assert_eq!(hooks.len(), 2);
    }

    #[test]
    fn extract_hooks_none() {
        let src = "function foo() { return 1; }";
        let conv = converter();
        assert!(conv.extract_hook_calls(src).is_empty());
    }

    // ── convert_file: Hello Dolly ────────────────────────────

    #[test]
    fn convert_hello_dolly_contains_plugin_header() {
        let src = r#"<?php
/*
Plugin Name: Hello Dolly
Version: 1.7.2
*/
function hello_dolly_get_lyric() {}
add_action( 'admin_notices', 'hello_dolly' );
"#;
        let file = php_parser::analyze_file(
            std::path::Path::new("hello.php"),
            src,
        )
        .unwrap();
        let conv = converter();
        let (code, _) = conv.convert_file(&file);
        assert!(code.contains("Plugin: Hello Dolly"));
        assert!(code.contains("Version: 1.7.2"));
    }

    #[test]
    fn convert_hello_dolly_hook_comment() {
        let src = r#"<?php
function hello_dolly() {}
add_action( 'admin_notices', 'hello_dolly' );
"#;
        let file = php_parser::analyze_file(
            std::path::Path::new("hello.php"),
            src,
        )
        .unwrap();
        let (code, _) = converter().convert_file(&file);
        assert!(code.contains("hooks::add_action"));
        assert!(code.contains("admin_notices"));
    }

    #[test]
    fn convert_function_name_snake_case() {
        let src = "<?php\nfunction helloWorld() { return 1; }\n";
        let file =
            php_parser::analyze_file(std::path::Path::new("t.php"), src).unwrap();
        let (code, _) = converter().convert_file(&file);
        assert!(code.contains("fn hello_world"));
    }

    #[test]
    fn convert_typed_params() {
        let src = "<?php\nfunction greet(string $name, int $age) { }\n";
        let file =
            php_parser::analyze_file(std::path::Path::new("t.php"), src).unwrap();
        let (code, _) = converter().convert_file(&file);
        assert!(code.contains("name: String"));
        assert!(code.contains("age: i64"));
    }

    #[test]
    fn convert_return_type_bool() {
        let src = "<?php\nfunction is_ready(): bool { return false; }\n";
        let file =
            php_parser::analyze_file(std::path::Path::new("t.php"), src).unwrap();
        let (code, _) = converter().convert_file(&file);
        assert!(code.contains("-> bool"));
    }

    #[test]
    fn convert_nullable_return_type() {
        let src = "<?php\nfunction find(int $id): ?string { return null; }\n";
        let file =
            php_parser::analyze_file(std::path::Path::new("t.php"), src).unwrap();
        let (code, _) = converter().convert_file(&file);
        assert!(code.contains("-> Option<String>"));
    }

    #[test]
    fn todo_count_nonzero_for_complex_body() {
        let src = "<?php\nfunction save() { global $wpdb; $wpdb->insert('t', []); }\n";
        let file =
            php_parser::analyze_file(std::path::Path::new("t.php"), src).unwrap();
        let (_, todos) = converter().convert_file(&file);
        assert!(todos > 0);
    }

    #[test]
    fn convert_class_struct_generated() {
        let src = r#"<?php
class MyPlugin {
    public string $name;
    private int $count;
    public function getName(): string { return $this->name; }
}
"#;
        let file =
            php_parser::analyze_file(std::path::Path::new("t.php"), src).unwrap();
        let (code, _) = converter().convert_file(&file);
        assert!(code.contains("pub struct MyPlugin"));
        assert!(code.contains("pub name: String"));
        assert!(code.contains("count: i64"));
        assert!(code.contains("impl MyPlugin"));
        assert!(code.contains("fn get_name"));
    }
}
