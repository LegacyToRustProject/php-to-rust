use crate::context::ConversionProfile;
use crate::llm::LlmProvider;
use crate::prompt;
use anyhow::{Context, Result};
use php_parser::types::{PhpFile, PhpProject};
use std::path::{Path, PathBuf};
use tracing::info;

/// Result of converting a single file.
#[derive(Debug)]
pub struct ConvertedFile {
    pub original_path: PathBuf,
    pub rust_code: String,
    pub output_path: PathBuf,
}

/// Orchestrates the conversion of PHP code to Rust using an LLM.
pub struct Generator {
    llm: Box<dyn LlmProvider>,
    profile: Option<ConversionProfile>,
}

impl Generator {
    pub fn new(llm: Box<dyn LlmProvider>, profile: Option<ConversionProfile>) -> Self {
        Self { llm, profile }
    }

    /// Convert a single PHP file to Rust.
    pub async fn convert_file(&self, file: &PhpFile) -> Result<String> {
        let system_prompt = prompt::build_system_prompt(self.profile.as_ref());
        let user_prompt = prompt::build_file_prompt(file);

        info!(
            "Converting file: {} using {}",
            file.path.display(),
            self.llm.name()
        );

        let response = self
            .llm
            .generate(&system_prompt, &user_prompt)
            .await
            .context("LLM generation failed")?;

        prompt::extract_rust_code(&response)
            .ok_or_else(|| anyhow::anyhow!("No Rust code block found in LLM response"))
    }

    /// Convert an entire PHP project to a Rust crate.
    /// Returns the output directory path and all converted files.
    pub async fn convert_project(
        &self,
        project: &PhpProject,
        output_dir: &Path,
    ) -> Result<Vec<ConvertedFile>> {
        std::fs::create_dir_all(output_dir)?;

        let src_dir = output_dir.join("src");
        std::fs::create_dir_all(&src_dir)?;

        let mut converted = Vec::new();
        let mut module_names = Vec::new();

        for file in &project.files {
            info!("Converting: {}", file.path.display());

            match self.convert_file(file).await {
                Ok(rust_code) => {
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
                    converted.push(ConvertedFile {
                        original_path: file.path.clone(),
                        rust_code,
                        output_path,
                    });
                }
                Err(e) => {
                    tracing::warn!("Failed to convert {}: {}", file.path.display(), e);
                }
            }
        }

        // Generate lib.rs with module declarations
        let lib_content = module_names
            .iter()
            .map(|name| format!("pub mod {};", name))
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(src_dir.join("lib.rs"), &lib_content)?;

        // Generate Cargo.toml
        let crate_name = project
            .root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .replace(' ', "-")
            .to_lowercase();
        let cargo_toml = format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1"
"#,
            crate_name
        );
        std::fs::write(output_dir.join("Cargo.toml"), &cargo_toml)?;

        info!(
            "Converted {}/{} files successfully",
            converted.len(),
            project.files.len()
        );

        Ok(converted)
    }
}
