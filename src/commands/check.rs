use anyhow::{Context, Result};

use crate::modules::common::is_valid_project_path;
use crate::modules::{check::missing_dependencies, templates::Template};

fn format_dependencies(deps: &[crate::modules::common::Dependency]) -> String {
    deps.iter()
        .map(|dep| format!("{} -> install: {}", dep.name, dep.install_hint))
        .collect::<Vec<_>>()
        .join("\n")
}

fn load_template_from_path(path: &std::path::Path) -> Result<Template> {
    let meta = std::fs::read_to_string(path.join("template.toml"))
        .context("Failed to read template.toml")?;
    toml::from_str(&meta)
        .context("Failed to parse template.toml")
}

pub fn run() -> Result<()> {
    let cwd = std::env::current_dir()?;
    
    if !is_valid_project_path(&cwd) {
        cliclack::outro("You are not in a valid project directory")?;
        return Ok(());
    }

    let template = load_template_from_path(&cwd)?;
    let missing = missing_dependencies(&template);

    if missing.is_empty() {
        cliclack::outro("No missing dependencies!")?;
    } else {
        cliclack::outro_note("Missing dependencies!", &format_dependencies(&missing))?;
    }

    Ok(())
}
