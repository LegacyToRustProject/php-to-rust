pub mod analyzer;
pub mod detector;
pub mod types;

pub use analyzer::{analyze_file, analyze_project, scan_php_files};
pub use types::*;
