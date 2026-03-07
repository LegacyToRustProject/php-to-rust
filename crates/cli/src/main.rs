use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use tracing::info;

#[derive(Parser)]
#[command(name = "php-to-rust")]
#[command(about = "AI-powered PHP to Rust conversion")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Analyze a PHP project and print a structure report.
    Analyze {
        /// Path to the PHP project directory or file.
        path: PathBuf,
    },

    /// Convert an entire PHP project to a Rust crate.
    Convert {
        /// Path to the PHP project directory.
        path: PathBuf,

        /// Output directory for the generated Rust crate.
        #[arg(short, long, default_value = "output")]
        output: PathBuf,

        /// Conversion profile (generic, wordpress, laravel).
        #[arg(long, default_value = "generic")]
        profile: String,

        /// Run verification (cargo check + fix loop) after conversion.
        #[arg(long)]
        verify: bool,

        /// LLM provider to use.
        #[arg(long, default_value = "claude")]
        llm: String,

        /// Model name for the LLM provider.
        #[arg(long)]
        model: Option<String>,

        /// Maximum fix loop iterations.
        #[arg(long, default_value = "10")]
        max_fix_iterations: usize,

        /// Conversion mode: "llm" (default, requires ANTHROPIC_API_KEY) or
        /// "pattern" (LLM-free, deterministic pattern matching).
        #[arg(long, default_value = "llm")]
        mode: String,
    },

    /// Convert a single PHP file to Rust.
    ConvertFile {
        /// Path to the PHP file.
        path: PathBuf,

        /// Output file path (defaults to stdout).
        #[arg(short, long)]
        output: Option<PathBuf>,

        /// Conversion profile.
        #[arg(long, default_value = "generic")]
        profile: String,

        /// Conversion mode: "llm" (default, requires ANTHROPIC_API_KEY) or
        /// "pattern" (LLM-free, deterministic pattern matching).
        #[arg(long, default_value = "llm")]
        mode: String,

        /// LLM provider to use (only used in llm mode).
        #[arg(long, default_value = "claude")]
        llm: String,

        /// Model name for the LLM provider (only used in llm mode).
        #[arg(long)]
        model: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Analyze { path } => cmd_analyze(&path),
        Commands::Convert {
            path,
            output,
            profile,
            verify,
            llm: _,
            model,
            max_fix_iterations,
            mode,
        } => {
            cmd_convert(
                &path,
                &output,
                &profile,
                &mode,
                verify,
                model,
                max_fix_iterations,
            )
            .await
        }
        Commands::ConvertFile {
            path,
            output,
            profile,
            mode,
            llm: _,
            model,
        } => cmd_convert_file(&path, output.as_deref(), &profile, &mode, model).await,
    }
}

fn cmd_analyze(path: &Path) -> Result<()> {
    let project = php_parser::analyze_project(path)
        .with_context(|| format!("Failed to analyze {}", path.display()))?;

    println!("=== PHP Project Analysis ===");
    println!("Root: {}", project.root.display());
    println!("PHP Version: {}", project.version);
    println!(
        "Framework: {}",
        project
            .framework
            .as_ref()
            .map(|f| f.to_string())
            .unwrap_or_else(|| "None detected".to_string())
    );
    println!("Files: {}", project.files.len());
    println!();

    let total_classes: usize = project.files.iter().map(|f| f.classes.len()).sum();
    let total_functions: usize = project.files.iter().map(|f| f.functions.len()).sum();
    let total_methods: usize = project
        .files
        .iter()
        .flat_map(|f| &f.classes)
        .map(|c| c.methods.len())
        .sum();

    println!("Summary:");
    println!("  Classes: {}", total_classes);
    println!("  Top-level functions: {}", total_functions);
    println!("  Methods: {}", total_methods);
    println!();

    for file in &project.files {
        println!("  {}", file.path.display());
        if !file.classes.is_empty() {
            for class in &file.classes {
                print!("    class {}", class.name);
                if let Some(ref parent) = class.extends {
                    print!(" extends {}", parent);
                }
                println!(
                    " ({} methods, {} properties)",
                    class.methods.len(),
                    class.properties.len()
                );
            }
        }
        if !file.functions.is_empty() {
            for func in &file.functions {
                println!("    fn {}({})", func.name, func.params.len());
            }
        }
        if !file.dependencies.is_empty() {
            println!("    deps: {}", file.dependencies.join(", "));
        }
    }

    Ok(())
}

async fn cmd_convert(
    path: &Path,
    output: &Path,
    profile_name: &str,
    mode: &str,
    verify: bool,
    model: Option<String>,
    max_fix_iterations: usize,
) -> Result<()> {
    let project = php_parser::analyze_project(path)
        .with_context(|| format!("Failed to analyze {}", path.display()))?;

    info!(
        "Analyzed project: {} files, {:?} framework",
        project.files.len(),
        project.framework
    );

    let profile = load_profile(profile_name)?;

    if mode == "pattern" {
        info!("Using PatternConverter (LLM-free)");
        let converter = rust_generator::PatternConverter::new(profile);
        let results = converter.convert_project(&project, output)?;
        let total_todos: usize = results.iter().map(|r| r.todos).sum();
        println!(
            "Converted {}/{} files to {} ({} TODO items)",
            results.len(),
            project.files.len(),
            output.display(),
            total_todos
        );
        if verify {
            println!("Note: --verify skipped in pattern mode (todo!() stubs present)");
        }
    } else {
        let llm = create_llm_provider(model.clone())?;
        let generator = rust_generator::Generator::new(llm, profile);
        let converted = generator.convert_project(&project, output).await?;
        println!(
            "Converted {}/{} files to {}",
            converted.len(),
            project.files.len(),
            output.display()
        );

        if verify {
            info!("Running verification...");
            let compile_result = verifier::cargo_check(output)?;
            match compile_result {
                verifier::CompileResult::Success => {
                    println!("Verification: compilation PASSED");
                }
                verifier::CompileResult::Errors(ref errors) => {
                    println!("Verification: compilation FAILED ({} errors)", errors.len());

                    if max_fix_iterations > 0 {
                        println!(
                            "Running fix loop (max {} iterations)...",
                            max_fix_iterations
                        );
                        let fix_llm = create_llm_provider(None)?;
                        let fix_loop = verifier::FixLoop::new(fix_llm, max_fix_iterations);

                        for file in &converted {
                            let code = std::fs::read_to_string(&file.output_path)?;
                            let (fixed, iters) =
                                fix_loop.run(&code, output, &file.output_path).await?;
                            std::fs::write(&file.output_path, &fixed)?;
                            println!(
                                "  Fixed {} in {} iterations",
                                file.output_path.display(),
                                iters
                            );
                        }

                        match verifier::cargo_check(output)? {
                            verifier::CompileResult::Success => {
                                println!("Verification after fixes: compilation PASSED");
                            }
                            verifier::CompileResult::Errors(errors) => {
                                println!(
                                    "Verification after fixes: still {} errors remaining",
                                    errors.len()
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

async fn cmd_convert_file(
    path: &PathBuf,
    output: Option<&std::path::Path>,
    profile_name: &str,
    mode: &str,
    model: Option<String>,
) -> Result<()> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.display()))?;

    let php_file = php_parser::analyze_file(path, &source)?;
    let profile = load_profile(profile_name)?;

    let rust_code = if mode == "pattern" {
        info!("Using PatternConverter (LLM-free) for {}", path.display());
        let converter = rust_generator::PatternConverter::new(profile);
        let (code, todos) = converter.convert_file(&php_file);
        info!("Pattern conversion complete: {} TODO items", todos);
        code
    } else {
        let llm = create_llm_provider(model)?;
        let generator = rust_generator::Generator::new(llm, profile);
        generator.convert_file(&php_file).await?
    };

    if let Some(out_path) = output {
        std::fs::create_dir_all(out_path.parent().unwrap_or(std::path::Path::new(".")))?;
        std::fs::write(out_path, &rust_code)?;
        println!("Written to {}", out_path.display());
    } else {
        println!("{}", rust_code);
    }

    Ok(())
}

fn load_profile(name: &str) -> Result<Option<rust_generator::ConversionProfile>> {
    if name == "generic" {
        return Ok(None);
    }

    // Look for profile in profiles/ directory relative to CWD, then relative to binary
    let candidates = [
        PathBuf::from(format!("profiles/{}", name)),
        PathBuf::from(format!("../profiles/{}", name)),
    ];

    for path in &candidates {
        if path.exists() {
            let profile = rust_generator::ConversionProfile::load(path)?;
            info!("Loaded profile '{}' from {}", name, path.display());
            return Ok(Some(profile));
        }
    }

    anyhow::bail!("Profile '{}' not found. Searched: {:?}", name, candidates);
}

fn create_llm_provider(model: Option<String>) -> Result<Box<dyn rust_generator::LlmProvider>> {
    let mut provider = rust_generator::ClaudeProvider::from_env()?;
    if let Some(model_name) = model {
        provider =
            rust_generator::ClaudeProvider::new(std::env::var("ANTHROPIC_API_KEY")?, model_name);
    }
    Ok(Box::new(provider))
}
