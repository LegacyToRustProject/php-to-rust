use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Detected PHP version range.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PhpVersion {
    Php5,
    Php7,
    Php8,
    Unknown,
}

impl std::fmt::Display for PhpVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Php5 => write!(f, "PHP 5.x"),
            Self::Php7 => write!(f, "PHP 7.x"),
            Self::Php8 => write!(f, "PHP 8.x"),
            Self::Unknown => write!(f, "Unknown"),
        }
    }
}

/// Detected framework.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Framework {
    WordPress,
    Laravel,
    Symfony,
    Generic,
}

impl std::fmt::Display for Framework {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::WordPress => write!(f, "WordPress"),
            Self::Laravel => write!(f, "Laravel"),
            Self::Symfony => write!(f, "Symfony"),
            Self::Generic => write!(f, "Generic PHP"),
        }
    }
}

/// Top-level representation of a PHP project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpProject {
    pub root: PathBuf,
    pub version: PhpVersion,
    pub framework: Option<Framework>,
    pub files: Vec<PhpFile>,
}

/// A single PHP source file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpFile {
    pub path: PathBuf,
    pub source: String,
    pub classes: Vec<PhpClass>,
    pub functions: Vec<PhpFunction>,
    pub dependencies: Vec<String>,
}

/// A PHP class definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpClass {
    pub name: String,
    pub extends: Option<String>,
    pub implements: Vec<String>,
    pub methods: Vec<PhpFunction>,
    pub properties: Vec<PhpProperty>,
}

/// A PHP function or method.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpFunction {
    pub name: String,
    pub params: Vec<PhpParam>,
    pub return_type: Option<String>,
    pub body: String,
    pub is_static: bool,
    pub visibility: Visibility,
}

/// A PHP class property.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpProperty {
    pub name: String,
    pub type_hint: Option<String>,
    pub visibility: Visibility,
    pub is_static: bool,
    pub default_value: Option<String>,
}

/// A PHP function parameter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhpParam {
    pub name: String,
    pub type_hint: Option<String>,
    pub default_value: Option<String>,
}

/// Visibility modifier.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub enum Visibility {
    #[default]
    Public,
    Protected,
    Private,
}
