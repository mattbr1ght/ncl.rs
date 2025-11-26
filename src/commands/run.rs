use anyhow::{Context, Result};

use crate::modules::cleanup;
use crate::modules::templates::{Template, run_hook};

use std::fs;
use std::path::PathBuf;

fn find_project_root_with_template() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        if dir.join("template.toml").is_file() {
            return Some(dir);
        }
        if !dir.pop() {
            break;
        }
    }
    None
}

fn list_available_hooks(hooks: &std::collections::HashMap<String, Vec<String>>) {
    for (name, commands) in hooks {
        println!("{}", name);
        for command in commands {
            println!("    {}", command);
        }
    }
}

pub fn run(job: Option<String>) -> Result<()> {
    let project_root = find_project_root_with_template().ok_or_else(|| {
        anyhow::anyhow!("Not in a project directory (no template.toml found above)")
    })?;
    let meta = fs::read_to_string(project_root.join("template.toml"))
        .context("Failed to read template.toml")?;
    let mut template: Template = toml::from_str(&meta).context("Failed to parse template.toml")?;
    template.path = project_root.clone();

    template.hooks.insert("remove-project".to_string(), vec!["Removes the project from coolify and github".to_string()]);
    if template.hooks.is_empty() {
        cliclack::outro("No hooks defined in template.toml")?;
        return Ok(());
    }

    match job {
        None => {
            list_available_hooks(&template.hooks);
            Ok(())
        }
        Some(job_name) => {
            // Special handling for remove-project hook
            if job_name == "remove-project" {
                cleanup::remove_project(&project_root)?;
                Ok(())
            } else {
                match template.hooks.get(&job_name) {
                    Some(commands) => {
                        run_hook(commands, &project_root)?;
                        cliclack::outro("")?;
                        Ok(())
                    }
                    None => {
                        cliclack::outro(&format!("The hook '{}' does not exist", job_name))?;
                        Ok(())
                    }
                }
            }
        }
    }
}
