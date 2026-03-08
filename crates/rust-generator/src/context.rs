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
    /// Maps each PHP function name → category label (e.g. "Posts", "Hooks").
    /// Loaded from `function_categories.toml` if present.
    pub function_categories: HashMap<String, String>,
    /// Maps category label → injection priority (lower = injected first).
    /// Used by the prompt builder to respect token budgets.
    pub category_priority: HashMap<String, u32>,
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
        let mut function_categories = HashMap::new();
        let mut category_priority = HashMap::new();

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

        // Load function_categories.toml (optional — enhances prompt grouping)
        let cat_path = profile_dir.join("function_categories.toml");
        if cat_path.exists() {
            let content = std::fs::read_to_string(&cat_path)?;
            let table: toml::Table = content.parse()?;
            if let Some(cats) = table.get("categories").and_then(|v| v.as_table()) {
                for (fn_name, cat) in cats {
                    if let Some(s) = cat.as_str() {
                        function_categories.insert(fn_name.clone(), s.to_string());
                    }
                }
            }
            if let Some(prio) = table.get("priority").and_then(|v| v.as_table()) {
                for (cat_name, p) in prio {
                    if let Some(n) = p.as_integer() {
                        category_priority.insert(cat_name.clone(), n as u32);
                    }
                }
            }
        }

        Ok(Self {
            name,
            api_mappings,
            type_mappings,
            additional_instructions,
            function_categories,
            category_priority,
        })
    }

    /// Create a generic (empty) profile.
    pub fn generic() -> Self {
        Self {
            name: "generic".to_string(),
            api_mappings: HashMap::new(),
            type_mappings: HashMap::new(),
            additional_instructions: String::new(),
            function_categories: HashMap::new(),
            category_priority: HashMap::new(),
        }
    }

    /// Return the category label for a PHP function, if known.
    pub fn category_of(&self, php_fn: &str) -> Option<&str> {
        self.function_categories.get(php_fn).map(|s| s.as_str())
    }

    /// Return sorted category names by injection priority (ascending).
    pub fn categories_by_priority(&self) -> Vec<&str> {
        let mut cats: Vec<&str> = self
            .function_categories
            .values()
            .map(|s| s.as_str())
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        cats.sort_by_key(|c| self.category_priority.get(*c).copied().unwrap_or(u32::MAX));
        cats
    }

    /// Return all mappings grouped by category, sorted by priority.
    ///
    /// Returns `Vec<(category_name, Vec<(php_fn, rust_path)>)>` in priority order.
    pub fn mappings_by_category(&self) -> Vec<(String, Vec<(String, String)>)> {
        let mut grouped: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for (php_fn, rust_path) in &self.api_mappings {
            let cat = self
                .function_categories
                .get(php_fn)
                .cloned()
                .unwrap_or_else(|| "Uncategorized".to_string());
            grouped
                .entry(cat)
                .or_default()
                .push((php_fn.clone(), rust_path.clone()));
        }
        // Sort within each category alphabetically
        for entries in grouped.values_mut() {
            entries.sort_by(|a, b| a.0.cmp(&b.0));
        }
        // Sort categories by priority
        let mut result: Vec<(String, Vec<(String, String)>)> = grouped.into_iter().collect();
        result.sort_by_key(|(cat, _)| {
            self.category_priority
                .get(cat.as_str())
                .copied()
                .unwrap_or(u32::MAX)
        });
        result
    }

    /// Filter API mappings to only those whose PHP function name appears in `source`.
    ///
    /// Used for selective prompt injection: avoids injecting irrelevant mappings.
    /// Results are sorted by category priority then alphabetically.
    pub fn mappings_for_source<'a>(&'a self, source: &str) -> Vec<(&'a str, &'a str)> {
        let mut matched: Vec<(&'a str, &'a str)> = self
            .api_mappings
            .iter()
            .filter(|(php_fn, _)| source.contains(php_fn.as_str()))
            .map(|(php_fn, rust_path)| (php_fn.as_str(), rust_path.as_str()))
            .collect();
        // Sort by category priority then by function name
        matched.sort_by(|(a, _), (b, _)| {
            let a_prio = self
                .function_categories
                .get(*a)
                .and_then(|c| self.category_priority.get(c.as_str()))
                .copied()
                .unwrap_or(u32::MAX);
            let b_prio = self
                .function_categories
                .get(*b)
                .and_then(|c| self.category_priority.get(c.as_str()))
                .copied()
                .unwrap_or(u32::MAX);
            a_prio.cmp(&b_prio).then_with(|| a.cmp(b))
        });
        matched
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
        assert!(profile.function_categories.is_empty());
        assert!(profile.category_priority.is_empty());
    }

    #[test]
    fn test_category_of_returns_none_for_generic() {
        let profile = ConversionProfile::generic();
        assert!(profile.category_of("add_action").is_none());
    }

    #[test]
    fn test_mappings_by_category_empty_profile() {
        let profile = ConversionProfile::generic();
        assert!(profile.mappings_by_category().is_empty());
    }

    #[test]
    fn test_mappings_for_source_filters_correctly() {
        let mut profile = ConversionProfile::generic();
        profile
            .api_mappings
            .insert("get_option".to_string(), "options::get".to_string());
        profile
            .api_mappings
            .insert("add_action".to_string(), "hooks::add_action".to_string());
        profile
            .api_mappings
            .insert("wp_insert_post".to_string(), "posts::insert".to_string());

        let source = "get_option('blogname'); add_action('init', 'my_func');";
        let matches = profile.mappings_for_source(source);
        let fn_names: Vec<&str> = matches.iter().map(|(f, _)| *f).collect();
        assert!(fn_names.contains(&"get_option"));
        assert!(fn_names.contains(&"add_action"));
        assert!(!fn_names.contains(&"wp_insert_post"));
    }

    #[test]
    fn test_categories_by_priority_with_data() {
        let mut profile = ConversionProfile::generic();
        profile
            .function_categories
            .insert("add_action".to_string(), "Hooks".to_string());
        profile
            .function_categories
            .insert("get_option".to_string(), "Options".to_string());
        profile.category_priority.insert("Hooks".to_string(), 1);
        profile.category_priority.insert("Options".to_string(), 3);

        let ordered = profile.categories_by_priority();
        let hooks_pos = ordered.iter().position(|c| *c == "Hooks").unwrap();
        let opts_pos = ordered.iter().position(|c| *c == "Options").unwrap();
        assert!(hooks_pos < opts_pos, "Hooks should come before Options");
    }

    #[test]
    fn test_mappings_for_source_respects_priority_order() {
        let mut profile = ConversionProfile::generic();
        profile
            .api_mappings
            .insert("get_option".to_string(), "options::get".to_string());
        profile
            .api_mappings
            .insert("add_action".to_string(), "hooks::add_action".to_string());
        profile
            .function_categories
            .insert("get_option".to_string(), "Options".to_string());
        profile
            .function_categories
            .insert("add_action".to_string(), "Hooks".to_string());
        profile.category_priority.insert("Hooks".to_string(), 1);
        profile.category_priority.insert("Options".to_string(), 3);

        let source = "add_action('init', fn); get_option('blogname');";
        let matches = profile.mappings_for_source(source);
        let fn_names: Vec<&str> = matches.iter().map(|(f, _)| *f).collect();
        let add_action_pos = fn_names.iter().position(|f| *f == "add_action").unwrap();
        let get_option_pos = fn_names.iter().position(|f| *f == "get_option").unwrap();
        assert!(add_action_pos < get_option_pos);
    }
}
