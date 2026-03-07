use crate::compiler::{self, CompileResult};
use anyhow::Result;
use rust_generator::LlmProvider;
use rust_generator::prompt;
use std::path::Path;
use tracing::{info, warn};

/// Controls the AI fix loop: compile → check errors → fix → repeat.
pub struct FixLoop {
    llm: Box<dyn LlmProvider>,
    max_iterations: usize,
}

impl FixLoop {
    pub fn new(llm: Box<dyn LlmProvider>, max_iterations: usize) -> Self {
        Self {
            llm,
            max_iterations,
        }
    }

    /// Run the fix loop on a single Rust source file within a project.
    ///
    /// Returns the fixed Rust code and the number of iterations used.
    pub async fn run(
        &self,
        rust_code: &str,
        project_dir: &Path,
        source_file: &Path,
    ) -> Result<(String, usize)> {
        let mut current_code = rust_code.to_string();

        for iteration in 1..=self.max_iterations {
            info!("Fix loop iteration {}/{}", iteration, self.max_iterations);

            // Write current code to file
            std::fs::write(source_file, &current_code)?;

            // Run cargo check
            let result = compiler::cargo_check(project_dir)?;

            match result {
                CompileResult::Success => {
                    info!("Compilation succeeded on iteration {}", iteration);
                    return Ok((current_code, iteration));
                }
                CompileResult::Errors(ref errors) => {
                    let error_msg = compiler::format_errors_for_llm(errors);
                    warn!(
                        "Compilation failed with {} errors, asking LLM to fix",
                        errors.len()
                    );

                    let system = "You are a Rust expert. Fix the compilation errors in the provided code. \
                                  Output ONLY the complete fixed Rust code inside a ```rust code block.";

                    let user = format!(
                        "The following Rust code has compilation errors. Fix them.\n\n\
                         Current code:\n```rust\n{}\n```\n\n{}",
                        current_code, error_msg
                    );

                    let response = self.llm.generate(system, &user).await?;

                    match prompt::extract_rust_code(&response) {
                        Some(fixed_code) => {
                            current_code = fixed_code;
                        }
                        None => {
                            warn!("LLM did not return a valid Rust code block");
                            // Continue with current code, hoping for a better response next time
                        }
                    }
                }
            }
        }

        warn!(
            "Fix loop exhausted after {} iterations",
            self.max_iterations
        );
        // Write final version and return
        std::fs::write(source_file, &current_code)?;
        Ok((current_code, self.max_iterations))
    }

    /// Run the fix loop with output comparison feedback.
    pub async fn run_with_output_feedback(
        &self,
        rust_code: &str,
        error_description: &str,
    ) -> Result<String> {
        let system = "You are a Rust expert. The Rust code compiles but produces incorrect output. \
                      Fix the logic to match the expected output. \
                      Output ONLY the complete fixed Rust code inside a ```rust code block.";

        let user = format!(
            "Fix this Rust code to produce the correct output:\n\n\
             ```rust\n{}\n```\n\n{}",
            rust_code, error_description
        );

        let response = self.llm.generate(system, &user).await?;

        prompt::extract_rust_code(&response)
            .ok_or_else(|| anyhow::anyhow!("LLM did not return valid Rust code for fix"))
    }
}
