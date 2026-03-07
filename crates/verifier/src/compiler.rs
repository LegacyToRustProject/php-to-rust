use anyhow::Result;
use std::path::Path;
use std::process::Command;

/// Result of a compilation check.
#[derive(Debug)]
pub enum CompileResult {
    Success,
    Errors(Vec<CompileError>),
}

/// A structured compilation error.
#[derive(Debug, Clone)]
pub struct CompileError {
    pub file: String,
    pub line: Option<usize>,
    pub message: String,
    pub suggestion: Option<String>,
}

impl std::fmt::Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(line) = self.line {
            write!(f, "{}:{}: {}", self.file, line, self.message)?;
        } else {
            write!(f, "{}: {}", self.file, self.message)?;
        }
        if let Some(ref suggestion) = self.suggestion {
            write!(f, " (suggestion: {})", suggestion)?;
        }
        Ok(())
    }
}

/// Check if a Rust project compiles successfully.
pub fn cargo_check(project_dir: &Path) -> Result<CompileResult> {
    let output = Command::new("cargo")
        .arg("check")
        .arg("--message-format=short")
        .current_dir(project_dir)
        .output()?;

    if output.status.success() {
        return Ok(CompileResult::Success);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors = parse_cargo_errors(&stderr);

    Ok(CompileResult::Errors(errors))
}

/// Parse cargo check stderr output into structured errors.
fn parse_cargo_errors(stderr: &str) -> Vec<CompileError> {
    let mut errors = Vec::new();
    let error_re = regex::Regex::new(r"error(?:\[E\d+\])?: (.+)").unwrap();
    let location_re = regex::Regex::new(r"^\s*--> (.+):(\d+):(\d+)").unwrap();

    let lines: Vec<&str> = stderr.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        if let Some(cap) = error_re.captures(lines[i]) {
            let message = cap[1].to_string();
            let mut file = String::new();
            let mut line = None;

            // Look for location on next lines
            for next_line in lines.iter().skip(i + 1).take(4) {
                if let Some(loc_cap) = location_re.captures(next_line) {
                    file = loc_cap[1].to_string();
                    line = loc_cap[2].parse().ok();
                    break;
                }
            }

            // Look for suggestion (help: ...)
            let suggestion = lines[i..lines.len().min(i + 10)]
                .iter()
                .find(|l| l.trim_start().starts_with("help:"))
                .map(|l| {
                    l.trim_start()
                        .strip_prefix("help: ")
                        .unwrap_or(l)
                        .to_string()
                });

            errors.push(CompileError {
                file,
                line,
                message,
                suggestion,
            });
        }
        i += 1;
    }

    // If no structured errors found, create one from the raw stderr
    if errors.is_empty() && !stderr.trim().is_empty() {
        errors.push(CompileError {
            file: String::new(),
            line: None,
            message: stderr.trim().to_string(),
            suggestion: None,
        });
    }

    errors
}

/// Format compilation errors for passing to the LLM fix loop.
pub fn format_errors_for_llm(errors: &[CompileError]) -> String {
    let mut output = String::from("The following compilation errors occurred:\n\n");
    for (i, error) in errors.iter().enumerate() {
        output.push_str(&format!("{}. {}\n", i + 1, error));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cargo_errors() {
        let stderr = r#"error[E0308]: mismatched types
 --> src/main.rs:5:12
  |
5 |     let x: i32 = "hello";
  |            ---   ^^^^^^^ expected `i32`, found `&str`
  |            |
  |            expected due to this

error: aborting due to 1 previous error
"#;
        let errors = parse_cargo_errors(stderr);
        assert!(!errors.is_empty());
        assert!(errors[0].message.contains("mismatched types"));
        assert_eq!(errors[0].file, "src/main.rs");
        assert_eq!(errors[0].line, Some(5));
    }
}
