use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;

use crate::modules::scaffold::ProjectOptions;

/// Trait for installable components (templates, etc.)
pub trait Installable {
    fn install(&self, project_options: &ProjectOptions) -> Result<()>;
}

/// Returns the NCL configuration directory path
pub fn ncl_config_dir() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("ncl")
}

/// Checks if a path is a valid NCL project (contains template.toml)
pub fn is_valid_project_path(path: &std::path::Path) -> bool {
    path.join("template.toml").exists()
}

/// Represents a dependency requirement for a template
#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(default)]
pub struct Dependency {
    pub name: String,
    pub check: String,
    pub install_hint: String,
}

impl Default for Dependency {
    fn default() -> Self {
        Self {
            name: String::new(),
            check: String::new(),
            install_hint: String::new(),
        }
    }
}
