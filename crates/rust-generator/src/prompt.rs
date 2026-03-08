use crate::context::ConversionProfile;
use php_parser::types::{PhpClass, PhpFile, PhpFunction};

// =============================================================================
// System prompt builders
// =============================================================================

/// Build the system prompt for PHP→Rust conversion.
///
/// Injects **all** API and type mappings from the profile. Use this when the
/// source code is not yet available (e.g. project-level prompts).
pub fn build_system_prompt(profile: Option<&ConversionProfile>) -> String {
    let mut prompt = base_rules();

    if let Some(profile) = profile {
        inject_framework_instructions(&mut prompt, profile);
        inject_mappings_grouped(&mut prompt, profile);
        inject_type_mappings(&mut prompt, &profile.type_mappings);
    }

    prompt
}

/// Build the system prompt with **selective** mapping injection.
///
/// Only injects mappings for functions that appear in `source_code`, grouped by
/// category and sorted by priority. This keeps the prompt compact and focused,
/// which is critical when the mapping set is large (500+ functions).
///
/// # Arguments
/// * `profile` — conversion profile with all mappings and category metadata
/// * `source_code` — the PHP source being converted (used to detect referenced functions)
/// * `max_mappings` — cap on injected mapping entries (0 = no limit)
pub fn build_system_prompt_selective(
    profile: &ConversionProfile,
    source_code: &str,
    max_mappings: usize,
) -> String {
    let mut prompt = base_rules();
    inject_framework_instructions(&mut prompt, profile);

    let matched = profile.mappings_for_source(source_code);

    let to_inject: Vec<(&str, &str)> = if max_mappings > 0 {
        matched.into_iter().take(max_mappings).collect()
    } else {
        matched
    };

    if !to_inject.is_empty() {
        prompt.push_str(
            "\n## API Mappings (detected in source)\n\nUse these Rust equivalents:\n\n",
        );

        // Group the selected mappings by category for readability
        let mut by_cat: std::collections::BTreeMap<String, Vec<(&str, &str)>> =
            std::collections::BTreeMap::new();
        for (php_fn, rust_path) in &to_inject {
            let cat = profile
                .category_of(php_fn)
                .unwrap_or("Other")
                .to_string();
            by_cat.entry(cat).or_default().push((php_fn, rust_path));
        }

        for (cat, entries) in &by_cat {
            prompt.push_str(&format!("### {cat}\n"));
            for (php_fn, rust_path) in entries {
                prompt.push_str(&format!("- `{php_fn}` → `{rust_path}`\n"));
            }
            prompt.push('\n');
        }
    }

    inject_type_mappings(&mut prompt, &profile.type_mappings);
    prompt
}

// =============================================================================
// User prompt builders
// =============================================================================

/// Build the user prompt for converting a single PHP file.
pub fn build_file_prompt(file: &PhpFile) -> String {
    let mut prompt = format!(
        "Convert the following PHP file to Rust.\n\nFile: {}\n\n",
        file.path.display()
    );

    if !file.dependencies.is_empty() {
        prompt.push_str("Dependencies (use/require):\n");
        for dep in &file.dependencies {
            prompt.push_str(&format!("- {}\n", dep));
        }
        prompt.push('\n');
    }

    prompt.push_str("```php\n");
    prompt.push_str(&file.source);
    prompt.push_str("\n```\n");

    prompt
}

/// Build the user prompt for converting a single PHP file with selective mapping injection.
///
/// Combines `build_file_prompt` with `build_system_prompt_selective` in one call
/// so callers don't need to split system/user prompts themselves.
pub fn build_file_prompt_with_selective_mappings(
    file: &PhpFile,
    profile: &ConversionProfile,
    max_mappings: usize,
) -> (String, String) {
    let system = build_system_prompt_selective(profile, &file.source, max_mappings);
    let user = build_file_prompt(file);
    (system, user)
}

/// Build a prompt for converting a single function.
pub fn build_function_prompt(func: &PhpFunction, context: &str) -> String {
    let mut prompt = format!(
        "Convert this PHP function to Rust:\n\n```php\nfunction {}(",
        func.name
    );

    for (i, param) in func.params.iter().enumerate() {
        if i > 0 {
            prompt.push_str(", ");
        }
        if let Some(ref t) = param.type_hint {
            prompt.push_str(&format!("{} ", t));
        }
        prompt.push_str(&format!("${}", param.name));
        if let Some(ref default) = param.default_value {
            prompt.push_str(&format!(" = {}", default));
        }
    }

    prompt.push(')');
    if let Some(ref ret) = func.return_type {
        prompt.push_str(&format!(": {}", ret));
    }
    prompt.push_str(" {\n");
    prompt.push_str(&func.body);
    prompt.push_str("\n}\n```\n");

    if !context.is_empty() {
        prompt.push_str(&format!("\nAdditional context:\n{}\n", context));
    }

    prompt
}

/// Build a prompt for converting a PHP class.
pub fn build_class_prompt(class: &PhpClass, context: &str) -> String {
    let mut prompt = format!(
        "Convert this PHP class to Rust (struct + impl):\n\n```php\nclass {}",
        class.name
    );

    if let Some(ref parent) = class.extends {
        prompt.push_str(&format!(" extends {}", parent));
    }
    if !class.implements.is_empty() {
        prompt.push_str(&format!(" implements {}", class.implements.join(", ")));
    }
    prompt.push_str(" {\n");

    for prop in &class.properties {
        let vis = match prop.visibility {
            php_parser::types::Visibility::Private => "private",
            php_parser::types::Visibility::Protected => "protected",
            php_parser::types::Visibility::Public => "public",
        };
        if let Some(ref t) = prop.type_hint {
            prompt.push_str(&format!("    {} {} ${};\n", vis, t, prop.name));
        } else {
            prompt.push_str(&format!("    {} ${};\n", vis, prop.name));
        }
    }

    for method in &class.methods {
        let vis = match method.visibility {
            php_parser::types::Visibility::Private => "private",
            php_parser::types::Visibility::Protected => "protected",
            php_parser::types::Visibility::Public => "public",
        };
        prompt.push_str(&format!("\n    {} function {}(", vis, method.name));
        for (i, p) in method.params.iter().enumerate() {
            if i > 0 {
                prompt.push_str(", ");
            }
            if let Some(ref t) = p.type_hint {
                prompt.push_str(&format!("{} ", t));
            }
            prompt.push_str(&format!("${}", p.name));
        }
        prompt.push(')');
        if let Some(ref ret) = method.return_type {
            prompt.push_str(&format!(": {}", ret));
        }
        prompt.push_str(" {\n");
        prompt.push_str(&format!("        {}\n", method.body.trim()));
        prompt.push_str("    }\n");
    }

    prompt.push_str("}\n```\n");

    if !context.is_empty() {
        prompt.push_str(&format!("\nAdditional context:\n{}\n", context));
    }

    prompt
}

// =============================================================================
// Response parsing
// =============================================================================

/// Extract Rust code from an LLM response (looks for ```rust blocks).
pub fn extract_rust_code(response: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?ms)```rust\s*\n(.*?)```").unwrap();
    re.captures(response).map(|cap| cap[1].trim().to_string())
}

// =============================================================================
// Private helpers
// =============================================================================

fn base_rules() -> String {
    String::from(
        r#"You are an expert PHP to Rust conversion engineer.

Your task is to convert PHP code into idiomatic, safe Rust code that preserves the same behavior.

## Rules

1. Write idiomatic Rust: use ownership, borrowing, Result/Option, traits, and enums appropriately.
2. Convert PHP classes to Rust structs + impl blocks.
3. Convert PHP arrays to Vec, HashMap, or appropriate Rust collections.
4. Convert PHP exceptions to Rust Result<T, E> with anyhow or thiserror.
5. Convert PHP nullable types (?Type) to Option<Type>.
6. Use &str or String appropriately (prefer &str for function parameters).
7. Add #[derive(Debug, Clone)] to structs where appropriate.
8. Mark anything you cannot convert with a `// TODO:` comment explaining why.
9. Do NOT add main() unless the PHP code is a standalone script.
10. Output ONLY the Rust code inside a ```rust code block. No explanations outside the code block.
"#,
    )
}

fn inject_framework_instructions(prompt: &mut String, profile: &ConversionProfile) {
    if !profile.additional_instructions.is_empty() {
        prompt.push_str("\n## Framework-Specific Instructions\n\n");
        prompt.push_str(&profile.additional_instructions);
        prompt.push('\n');
    }
}

/// Inject all API mappings grouped by category (for full-mapping prompts).
fn inject_mappings_grouped(prompt: &mut String, profile: &ConversionProfile) {
    let by_cat = profile.mappings_by_category();
    if by_cat.is_empty() {
        return;
    }

    prompt.push_str("\n## API Mappings\n\nUse these Rust equivalents for framework functions:\n");

    for (cat, entries) in &by_cat {
        if cat == "Uncategorized" {
            prompt.push_str("\n### Other\n");
        } else {
            prompt.push_str(&format!("\n### {cat}\n"));
        }
        for (php_fn, rust_path) in entries {
            prompt.push_str(&format!("- `{php_fn}` → `{rust_path}`\n"));
        }
    }
    prompt.push('\n');
}

fn inject_type_mappings(prompt: &mut String, type_mappings: &std::collections::HashMap<String, String>) {
    if type_mappings.is_empty() {
        return;
    }
    prompt.push_str("\n## Type Mappings\n\n");
    let mut types: Vec<(&String, &String)> = type_mappings.iter().collect();
    types.sort_by_key(|(k, _)| k.as_str());
    for (php_type, rust_type) in types {
        prompt.push_str(&format!("- `{php_type}` → `{rust_type}`\n"));
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_profile_with_mappings() -> ConversionProfile {
        let mut profile = ConversionProfile::generic();
        profile
            .api_mappings
            .insert("add_action".to_string(), "hooks::add_action".to_string());
        profile
            .api_mappings
            .insert("get_option".to_string(), "options::get".to_string());
        profile
            .api_mappings
            .insert("wp_insert_post".to_string(), "posts::insert".to_string());
        profile
            .api_mappings
            .insert("wp_remote_get".to_string(), "http::get".to_string());
        profile
            .function_categories
            .insert("add_action".to_string(), "Hooks".to_string());
        profile
            .function_categories
            .insert("get_option".to_string(), "Options".to_string());
        profile
            .function_categories
            .insert("wp_insert_post".to_string(), "Posts".to_string());
        profile
            .function_categories
            .insert("wp_remote_get".to_string(), "HTTP".to_string());
        profile.category_priority.insert("Hooks".to_string(), 1);
        profile.category_priority.insert("Posts".to_string(), 2);
        profile.category_priority.insert("Options".to_string(), 3);
        profile.category_priority.insert("HTTP".to_string(), 12);
        profile
    }

    // --- extract_rust_code ---------------------------------------------------

    #[test]
    fn test_extract_rust_code() {
        let response = r#"Here is the converted code:

```rust
fn add(a: i64, b: i64) -> i64 {
    a + b
}
```

This converts the PHP function to Rust."#;

        let code = extract_rust_code(response).unwrap();
        assert!(code.contains("fn add"));
        assert!(code.contains("a + b"));
    }

    #[test]
    fn test_extract_rust_code_none() {
        assert!(extract_rust_code("no code here").is_none());
    }

    // --- build_system_prompt -------------------------------------------------

    #[test]
    fn test_build_system_prompt_no_profile() {
        let prompt = build_system_prompt(None);
        assert!(prompt.contains("PHP to Rust"));
        assert!(prompt.contains("idiomatic"));
    }

    #[test]
    fn test_build_system_prompt_injects_all_mappings() {
        let profile = make_profile_with_mappings();
        let prompt = build_system_prompt(Some(&profile));
        assert!(prompt.contains("add_action"));
        assert!(prompt.contains("get_option"));
        assert!(prompt.contains("wp_insert_post"));
        assert!(prompt.contains("wp_remote_get"));
    }

    #[test]
    fn test_build_system_prompt_groups_by_category() {
        let profile = make_profile_with_mappings();
        let prompt = build_system_prompt(Some(&profile));
        // Category headers should appear
        assert!(prompt.contains("### Hooks") || prompt.contains("### Options"));
    }

    #[test]
    fn test_build_system_prompt_hooks_before_options() {
        let profile = make_profile_with_mappings();
        let prompt = build_system_prompt(Some(&profile));
        // Hooks (priority 1) section must appear before Options (priority 3)
        let hooks_pos = prompt.find("### Hooks").unwrap_or(usize::MAX);
        let opts_pos = prompt.find("### Options").unwrap_or(usize::MAX);
        assert!(hooks_pos < opts_pos, "Hooks section must come before Options section");
    }

    // --- build_system_prompt_selective ----------------------------------------

    #[test]
    fn test_selective_only_injects_detected_functions() {
        let profile = make_profile_with_mappings();
        let source = "add_action('init', fn); get_option('siteurl');";
        let prompt = build_system_prompt_selective(&profile, source, 0);
        assert!(prompt.contains("add_action"));
        assert!(prompt.contains("get_option"));
        // wp_insert_post not in source → should NOT appear
        assert!(!prompt.contains("wp_insert_post"));
        assert!(!prompt.contains("wp_remote_get"));
    }

    #[test]
    fn test_selective_respects_max_mappings() {
        let profile = make_profile_with_mappings();
        let source = "add_action('init', fn); get_option('x'); wp_insert_post([]); wp_remote_get('url');";
        let prompt = build_system_prompt_selective(&profile, source, 2);
        // Count how many mapping lines appear
        let mapping_lines = prompt.lines().filter(|l| l.contains(" → ")).count();
        assert!(mapping_lines <= 2, "Expected at most 2 mappings, got {mapping_lines}");
    }

    #[test]
    fn test_selective_max_zero_means_no_limit() {
        let profile = make_profile_with_mappings();
        let source = "add_action('init', fn); get_option('x'); wp_insert_post([]); wp_remote_get('url');";
        let prompt = build_system_prompt_selective(&profile, source, 0);
        let mapping_lines = prompt.lines().filter(|l| l.contains(" → ")).count();
        assert_eq!(mapping_lines, 4, "All 4 detected mappings should be injected");
    }

    #[test]
    fn test_selective_empty_source_injects_nothing() {
        let profile = make_profile_with_mappings();
        let prompt = build_system_prompt_selective(&profile, "", 0);
        assert!(!prompt.contains(" → "), "No mappings should be injected for empty source");
    }

    #[test]
    fn test_selective_contains_category_headers() {
        let profile = make_profile_with_mappings();
        let source = "add_action('init', fn); wp_remote_get('url');";
        let prompt = build_system_prompt_selective(&profile, source, 0);
        // Should include category headers for detected functions
        assert!(prompt.contains("### Hooks") || prompt.contains("Hooks"));
        assert!(prompt.contains("### HTTP") || prompt.contains("HTTP"));
    }

    // --- type mapping injection ----------------------------------------------

    #[test]
    fn test_type_mappings_included_in_system_prompt() {
        let mut profile = ConversionProfile::generic();
        profile
            .type_mappings
            .insert("WP_Post".to_string(), "Post".to_string());
        profile
            .type_mappings
            .insert("WP_Error".to_string(), "anyhow::Error".to_string());
        let prompt = build_system_prompt(Some(&profile));
        assert!(prompt.contains("WP_Post"));
        assert!(prompt.contains("Post"));
        assert!(prompt.contains("WP_Error"));
    }

    // --- file prompt with selective mappings ---------------------------------

    #[test]
    fn test_build_file_prompt_with_selective_mappings_returns_two_parts() {
        let profile = make_profile_with_mappings();
        let file = PhpFile {
            path: std::path::PathBuf::from("test.php"),
            source: "<?php add_action('init', 'myfunc');".to_string(),
            functions: vec![],
            classes: vec![],
            dependencies: vec![],
        };
        let (system, user) = build_file_prompt_with_selective_mappings(&file, &profile, 0);
        assert!(system.contains("add_action"), "system should contain detected mapping");
        assert!(!system.contains("wp_insert_post"), "unrelated mapping must not appear");
        assert!(user.contains("test.php"), "user prompt should contain filename");
        assert!(user.contains("add_action"), "user prompt should contain source code");
    }
}
