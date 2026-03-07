use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A conversion profile that provides framework-specific mappings and instructions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionProfile {
    pub name: String,
    pub api_mappings: HashMap<String, String>,
    pub type_mappings: HashMap<String, String>,
    pub additional_instructions: String,
}

impl ConversionProfile {
    /// Load a profile from a directory containing TOML files.
    pub fn load(profile_dir: &Path) -> Result<Self> {
        let name = profile_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        let mut api_mappings = HashMap::new();
        let mut type_mappings = HashMap::new();
        let mut additional_instructions = String::new();

        // Load api_mappings.toml
        let api_path = profile_dir.join("api_mappings.toml");
        if api_path.exists() {
            let content = std::fs::read_to_string(&api_path)?;
            let table: toml::Table = content.parse()?;
            if let Some(functions) = table.get("functions").and_then(|v| v.as_table()) {
                for (k, v) in functions {
                    if let Some(s) = v.as_str() {
                        api_mappings.insert(k.clone(), s.to_string());
                    }
                }
            }
            if let Some(types) = table.get("types").and_then(|v| v.as_table()) {
                for (k, v) in types {
                    if let Some(s) = v.as_str() {
                        type_mappings.insert(k.clone(), s.to_string());
                    }
                }
            }
        }

        // Load hooks.toml for additional patterns
        let hooks_path = profile_dir.join("hooks.toml");
        if hooks_path.exists() {
            let content = std::fs::read_to_string(&hooks_path)?;
            let table: toml::Table = content.parse()?;
            if let Some(patterns) = table.get("patterns").and_then(|v| v.as_table()) {
                for (k, v) in patterns {
                    if let Some(s) = v.as_str() {
                        api_mappings.insert(k.clone(), s.to_string());
                    }
                }
            }
            if let Some(instructions) = table.get("instructions").and_then(|v| v.as_str()) {
                additional_instructions.push_str(instructions);
                additional_instructions.push('\n');
            }
        }

        // Load db_patterns.toml
        let db_path = profile_dir.join("db_patterns.toml");
        if db_path.exists() {
            let content = std::fs::read_to_string(&db_path)?;
            let table: toml::Table = content.parse()?;
            if let Some(patterns) = table.get("patterns").and_then(|v| v.as_table()) {
                for (k, v) in patterns {
                    if let Some(s) = v.as_str() {
                        api_mappings.insert(k.clone(), s.to_string());
                    }
                }
            }
        }

        Ok(Self {
            name,
            api_mappings,
            type_mappings,
            additional_instructions,
        })
    }

    /// Create a generic (empty) profile.
    pub fn generic() -> Self {
        Self {
            name: "generic".to_string(),
            api_mappings: HashMap::new(),
            type_mappings: HashMap::new(),
            additional_instructions: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generic_profile() {
        let profile = ConversionProfile::generic();
        assert_eq!(profile.name, "generic");
        assert!(profile.api_mappings.is_empty());
    }
}
