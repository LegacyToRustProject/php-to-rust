use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Result of comparing PHP and Rust output.
#[derive(Debug)]
pub enum ComparisonResult {
    Match,
    Mismatch {
        php_output: String,
        rust_output: String,
        diff: String,
    },
    PhpError(String),
    RustError(String),
}

/// Compare the output of a PHP script with a Rust binary.
pub fn compare_outputs(
    php_file: &Path,
    rust_binary: &Path,
    php_binary: &str,
    args: &[&str],
) -> Result<ComparisonResult> {
    // Run PHP
    let php_result = Command::new(php_binary).arg(php_file).args(args).output();

    let php_output = match php_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return Ok(ComparisonResult::PhpError(stderr));
            }
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Err(e) => {
            return Ok(ComparisonResult::PhpError(format!(
                "Failed to run PHP: {}",
                e
            )));
        }
    };

    // Run Rust binary
    let rust_result = Command::new(rust_binary).args(args).output();

    let rust_output = match rust_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                return Ok(ComparisonResult::RustError(stderr));
            }
            String::from_utf8_lossy(&output.stdout).to_string()
        }
        Err(e) => {
            return Ok(ComparisonResult::RustError(format!(
                "Failed to run Rust binary: {}",
                e
            )));
        }
    };

    // Compare outputs
    if php_output.trim() == rust_output.trim() {
        Ok(ComparisonResult::Match)
    } else {
        let diff = generate_diff(&php_output, &rust_output);
        Ok(ComparisonResult::Mismatch {
            php_output,
            rust_output,
            diff,
        })
    }
}

/// Generate a simple line-by-line diff between two strings.
fn generate_diff(expected: &str, actual: &str) -> String {
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let mut diff = String::new();

    let max_lines = expected_lines.len().max(actual_lines.len());
    for i in 0..max_lines {
        let exp = expected_lines.get(i).unwrap_or(&"<missing>");
        let act = actual_lines.get(i).unwrap_or(&"<missing>");
        if exp != act {
            diff.push_str(&format!(
                "Line {}:\n  PHP:  {}\n  Rust: {}\n",
                i + 1,
                exp,
                act
            ));
        }
    }

    if diff.is_empty() {
        diff.push_str("(whitespace-only differences)");
    }

    diff
}

/// Format a comparison mismatch for the LLM fix loop.
pub fn format_mismatch_for_llm(php_output: &str, rust_output: &str, diff: &str) -> String {
    format!(
        r#"The Rust code produces different output than the PHP code.

Expected output (from PHP):
```
{}
```

Actual output (from Rust):
```
{}
```

Differences:
{}

Please fix the Rust code to produce the same output as the PHP code."#,
        php_output.trim(),
        rust_output.trim(),
        diff
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_diff() {
        let diff = generate_diff("hello\nworld", "hello\nearth");
        assert!(diff.contains("Line 2"));
        assert!(diff.contains("world"));
        assert!(diff.contains("earth"));
    }

    #[test]
    fn test_generate_diff_matching() {
        let diff = generate_diff("hello", "hello");
        assert!(diff.contains("whitespace"));
    }
}
