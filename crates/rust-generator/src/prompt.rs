use crate::context::ConversionProfile;
use php_parser::types::{PhpClass, PhpFile, PhpFunction};

/// Build the system prompt for PHP→Rust conversion.
pub fn build_system_prompt(profile: Option<&ConversionProfile>) -> String {
    let mut prompt = String::from(
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
    );

    if let Some(profile) = profile {
        prompt.push_str("\n## Framework-Specific Instructions\n\n");
        prompt.push_str(&profile.additional_instructions);
        prompt.push('\n');

        if !profile.api_mappings.is_empty() {
            prompt.push_str(
                "\n## API Mappings\n\nUse these Rust equivalents for framework functions:\n\n",
            );
            for (php_fn, rust_fn) in &profile.api_mappings {
                prompt.push_str(&format!("- `{}` → `{}`\n", php_fn, rust_fn));
            }
        }

        if !profile.type_mappings.is_empty() {
            prompt.push_str("\n## Type Mappings\n\n");
            for (php_type, rust_type) in &profile.type_mappings {
                prompt.push_str(&format!("- `{}` → `{}`\n", php_type, rust_type));
            }
        }
    }

    prompt
}

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

/// Extract Rust code from an LLM response (looks for ```rust blocks).
pub fn extract_rust_code(response: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?ms)```rust\s*\n(.*?)```").unwrap();
    re.captures(response).map(|cap| cap[1].trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_build_system_prompt_no_profile() {
        let prompt = build_system_prompt(None);
        assert!(prompt.contains("PHP to Rust"));
        assert!(prompt.contains("idiomatic"));
    }
}
