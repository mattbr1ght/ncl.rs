use serde::Deserialize;

use crate::modules::scaffold::ProjectOptions;
use std::path::PathBuf;

pub trait Installable {
    fn install(&self, project_options: &ProjectOptions) -> std::io::Result<()>;
}

pub fn ncl_config_dir() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from(".")) // fallback
        .join("ncl")
}

pub fn is_valid_project_path(path: &std::path::Path) -> bool {
    path.join("template.toml").exists()
}

// --- //

#[derive(Deserialize)]
#[derive(Debug)]
#[derive(Clone, Eq, PartialEq)]
#[serde(default = "Dependency::default")]
pub struct Dependency {
    pub name: String,
    pub check: String,
    pub install_hint: String,
}


impl Dependency {
    pub fn default() -> Self {
        Self {
            name: String::new(),
            check: String::new(),
            install_hint: String::new(),
        }
    }
}
