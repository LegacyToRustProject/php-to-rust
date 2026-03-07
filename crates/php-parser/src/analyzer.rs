use crate::types::*;
use anyhow::{Context, Result};
use regex::Regex;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;
use walkdir::WalkDir;

// Pre-compiled regexes — compiled once, never panic at call site.

static CLASS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ms)(abstract\s+)?class\s+(\w+)(?:\s+extends\s+(\w+))?(?:\s+implements\s+([\w,\s\\]+))?\s*\{",
    )
    .expect("invalid CLASS_RE")
});

static FUNC_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^function\s+(\w+)\s*\(([^)]*)\)(?:\s*:\s*(\??\w+(?:\|\w+)*))?\s*\{")
        .expect("invalid FUNC_RE")
});

static METHOD_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(public|protected|private)?\s*(static\s+)?function\s+(\w+)\s*\(([^)]*)\)(?:\s*:\s*(\??\w+(?:\|\w+)*))?\s*\{",
    )
    .expect("invalid METHOD_RE")
});

static PROP_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?m)(public|protected|private)\s+(static\s+)?(readonly\s+)?(?:(\w+)\s+)?\$(\w+)(?:\s*=\s*([^;]+))?\s*;",
    )
    .expect("invalid PROP_RE")
});

static USE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^use\s+([\w\\]+)").expect("invalid USE_RE"));

static REQUIRE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?:require|include)(?:_once)?\s*\(?\s*['"]([^'"]+)['"]\s*\)?"#)
        .expect("invalid REQUIRE_RE")
});

static PARAM_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:(\??\w+(?:\|\w+)*)\s+)?\$(\w+)").expect("invalid PARAM_RE"));

/// Scan a directory for all PHP files.
pub fn scan_php_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(root)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "php") {
            files.push(path.to_path_buf());
        }
    }
    files.sort();
    Ok(files)
}

/// Parse a single PHP file and extract structural information.
pub fn analyze_file(path: &Path, source: &str) -> Result<PhpFile> {
    let classes = extract_classes(source);
    let functions = extract_functions(source);
    let dependencies = extract_dependencies(source);

    Ok(PhpFile {
        path: path.to_path_buf(),
        source: source.to_string(),
        classes,
        functions,
        dependencies,
    })
}

/// Analyze an entire PHP project directory.
pub fn analyze_project(root: &Path) -> Result<PhpProject> {
    let php_paths = scan_php_files(root)?;
    let mut files = Vec::new();
    let mut all_sources: Vec<String> = Vec::new();

    for path in &php_paths {
        let source = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        all_sources.push(source.clone());
        let php_file = analyze_file(path, &source)?;
        files.push(php_file);
    }

    let source_refs: Vec<&str> = all_sources.iter().map(|s| s.as_str()).collect();
    let version = crate::detector::detect_version(&source_refs);
    let framework = crate::detector::detect_framework(&source_refs);

    Ok(PhpProject {
        root: root.to_path_buf(),
        version,
        framework,
        files,
    })
}

/// Extract class definitions from PHP source using regex.
fn extract_classes(source: &str) -> Vec<PhpClass> {
    let mut classes = Vec::new();

    for cap in CLASS_RE.captures_iter(source) {
        let name = cap[2].to_string();
        let extends = cap.get(3).map(|m| m.as_str().to_string());
        let implements = cap
            .get(4)
            .map(|m| {
                m.as_str()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        // cap.get(0) is always Some in captures_iter
        let full_match = cap.get(0).expect("full match always exists");
        let class_start = full_match.end() - 1;
        let class_body = extract_brace_block(source, class_start).unwrap_or_default();

        let methods = extract_methods(&class_body);
        let properties = extract_properties(&class_body);

        classes.push(PhpClass {
            name,
            extends,
            implements,
            methods,
            properties,
        });
    }

    classes
}

/// Extract top-level functions (not class methods) from PHP source.
fn extract_functions(source: &str) -> Vec<PhpFunction> {
    let mut functions = Vec::new();

    for cap in FUNC_RE.captures_iter(source) {
        let name = cap[1].to_string();
        let params_str = cap.get(2).map(|m| m.as_str()).unwrap_or("");
        let return_type = cap.get(3).map(|m| m.as_str().to_string());

        let full_match = cap.get(0).expect("full match always exists");
        let func_start = full_match.end() - 1;
        let body = extract_brace_block(source, func_start).unwrap_or_default();

        functions.push(PhpFunction {
            name,
            params: parse_params(params_str),
            return_type,
            body,
            is_static: false,
            visibility: Visibility::Public,
        });
    }

    functions
}

/// Extract methods from a class body.
fn extract_methods(class_body: &str) -> Vec<PhpFunction> {
    let mut methods = Vec::new();

    for cap in METHOD_RE.captures_iter(class_body) {
        let visibility = match cap.get(1).map(|m| m.as_str()) {
            Some("private") => Visibility::Private,
            Some("protected") => Visibility::Protected,
            _ => Visibility::Public,
        };
        let is_static = cap.get(2).is_some();
        let name = cap[3].to_string();
        let params_str = cap.get(4).map(|m| m.as_str()).unwrap_or("");
        let return_type = cap.get(5).map(|m| m.as_str().to_string());

        let full_match = cap.get(0).expect("full match always exists");
        let func_start = full_match.end() - 1;
        let body = extract_brace_block(class_body, func_start).unwrap_or_default();

        methods.push(PhpFunction {
            name,
            params: parse_params(params_str),
            return_type,
            body,
            is_static,
            visibility,
        });
    }

    methods
}

/// Extract class properties.
fn extract_properties(class_body: &str) -> Vec<PhpProperty> {
    let mut properties = Vec::new();

    for cap in PROP_RE.captures_iter(class_body) {
        let visibility = match &cap[1] {
            "private" => Visibility::Private,
            "protected" => Visibility::Protected,
            _ => Visibility::Public,
        };
        let is_static = cap.get(2).is_some();
        let type_hint = cap.get(4).map(|m| m.as_str().to_string());
        let name = cap[5].to_string();
        let default_value = cap.get(6).map(|m| m.as_str().trim().to_string());

        properties.push(PhpProperty {
            name,
            type_hint,
            visibility,
            is_static,
            default_value,
        });
    }

    properties
}

/// Extract dependencies (use, require, include statements).
fn extract_dependencies(source: &str) -> Vec<String> {
    let mut deps = Vec::new();

    for cap in USE_RE.captures_iter(source) {
        deps.push(cap[1].to_string());
    }

    for cap in REQUIRE_RE.captures_iter(source) {
        deps.push(cap[1].to_string());
    }

    deps
}

/// Parse a parameter list string into PhpParam structs.
fn parse_params(params_str: &str) -> Vec<PhpParam> {
    if params_str.trim().is_empty() {
        return Vec::new();
    }

    params_str
        .split(',')
        .filter_map(|p| {
            let p = p.trim();
            if p.is_empty() {
                return None;
            }

            let parts: Vec<&str> = p.split('=').collect();
            let default_value = parts.get(1).map(|v| v.trim().to_string());

            let decl = parts[0].trim();
            PARAM_RE.captures(decl).map(|cap| PhpParam {
                name: cap[2].to_string(),
                type_hint: cap.get(1).map(|m| m.as_str().to_string()),
                default_value,
            })
        })
        .collect()
}

/// Extract a brace-delimited block starting at the opening brace position.
fn extract_brace_block(source: &str, start: usize) -> Option<String> {
    let bytes = source.as_bytes();
    if start >= bytes.len() || bytes[start] != b'{' {
        return None;
    }

    let mut depth = 0;
    let mut in_string = false;
    let mut string_char = b'"';
    let mut i = start;

    while i < bytes.len() {
        let ch = bytes[i];

        if in_string {
            if ch == b'\\' {
                i += 1; // skip escaped char
            } else if ch == string_char {
                in_string = false;
            }
        } else {
            match ch {
                b'"' | b'\'' => {
                    in_string = true;
                    string_char = ch;
                }
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(source[start + 1..i].to_string());
                    }
                }
                _ => {}
            }
        }
        i += 1;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_functions() {
        let src = r#"<?php
function add($a, $b) {
    return $a + $b;
}

function greet(string $name): string {
    return "Hello, " . $name;
}
"#;
        let funcs = extract_functions(src);
        assert_eq!(funcs.len(), 2);
        assert_eq!(funcs[0].name, "add");
        assert_eq!(funcs[0].params.len(), 2);
        assert_eq!(funcs[1].name, "greet");
        assert_eq!(funcs[1].return_type.as_deref(), Some("string"));
    }

    #[test]
    fn test_extract_classes() {
        let src = r#"<?php
class User extends BaseModel implements Serializable {
    public string $name;
    private int $age;

    public function getName(): string {
        return $this->name;
    }

    protected function setAge(int $age): void {
        $this->age = $age;
    }
}
"#;
        let classes = extract_classes(src);
        assert_eq!(classes.len(), 1);
        assert_eq!(classes[0].name, "User");
        assert_eq!(classes[0].extends.as_deref(), Some("BaseModel"));
        assert_eq!(classes[0].implements, vec!["Serializable"]);
        assert_eq!(classes[0].methods.len(), 2);
        assert_eq!(classes[0].properties.len(), 2);
    }

    #[test]
    fn test_extract_dependencies() {
        let src = r#"<?php
use App\Models\User;
use Illuminate\Http\Request;
require_once 'vendor/autoload.php';
include 'helpers.php';
"#;
        let deps = extract_dependencies(src);
        assert_eq!(deps.len(), 4);
        assert!(deps.contains(&"App\\Models\\User".to_string()));
        assert!(deps.contains(&"vendor/autoload.php".to_string()));
    }

    #[test]
    fn test_parse_params() {
        let params = parse_params("int $x, string $name = 'default', $flag");
        assert_eq!(params.len(), 3);
        assert_eq!(params[0].name, "x");
        assert_eq!(params[0].type_hint.as_deref(), Some("int"));
        assert_eq!(params[1].name, "name");
        assert!(params[1].default_value.is_some());
        assert_eq!(params[2].name, "flag");
        assert!(params[2].type_hint.is_none());
    }
}
