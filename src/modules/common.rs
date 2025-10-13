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
