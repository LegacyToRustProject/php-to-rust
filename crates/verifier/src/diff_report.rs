use crate::comparator::ComparisonResult;
use crate::compiler::CompileError;

/// A verification report summarizing the results.
#[derive(Debug)]
pub struct VerificationReport {
    pub file_name: String,
    pub compile_status: CompileStatus,
    pub output_comparison: Option<OutputStatus>,
    pub iterations_used: usize,
}

#[derive(Debug)]
pub enum CompileStatus {
    Success,
    Failed(Vec<CompileError>),
}

#[derive(Debug)]
pub enum OutputStatus {
    Match,
    Mismatch(String),
    Skipped(String),
}

impl std::fmt::Display for VerificationReport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Verification Report: {} ===", self.file_name)?;
        writeln!(f, "Iterations used: {}", self.iterations_used)?;

        match &self.compile_status {
            CompileStatus::Success => writeln!(f, "Compilation: PASS")?,
            CompileStatus::Failed(errors) => {
                writeln!(f, "Compilation: FAIL")?;
                for error in errors {
                    writeln!(f, "  - {}", error)?;
                }
            }
        }

        if let Some(ref output) = self.output_comparison {
            match output {
                OutputStatus::Match => writeln!(f, "Output comparison: MATCH")?,
                OutputStatus::Mismatch(diff) => {
                    writeln!(f, "Output comparison: MISMATCH")?;
                    writeln!(f, "{}", diff)?;
                }
                OutputStatus::Skipped(reason) => {
                    writeln!(f, "Output comparison: SKIPPED ({})", reason)?;
                }
            }
        }

        Ok(())
    }
}

/// Build a report from verification results.
pub fn build_report(
    file_name: &str,
    compile_result: &crate::compiler::CompileResult,
    comparison: Option<&ComparisonResult>,
    iterations: usize,
) -> VerificationReport {
    let compile_status = match compile_result {
        crate::compiler::CompileResult::Success => CompileStatus::Success,
        crate::compiler::CompileResult::Errors(errors) => CompileStatus::Failed(errors.clone()),
    };

    let output_comparison = comparison.map(|c| match c {
        ComparisonResult::Match => OutputStatus::Match,
        ComparisonResult::Mismatch { diff, .. } => OutputStatus::Mismatch(diff.clone()),
        ComparisonResult::PhpError(e) => OutputStatus::Skipped(format!("PHP error: {}", e)),
        ComparisonResult::RustError(e) => OutputStatus::Skipped(format!("Rust error: {}", e)),
    });

    VerificationReport {
        file_name: file_name.to_string(),
        compile_status,
        output_comparison,
        iterations_used: iterations,
    }
}
