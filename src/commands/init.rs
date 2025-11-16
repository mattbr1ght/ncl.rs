use anyhow::{anyhow, Context, Result};
use console::style;
use log::debug;
use sanitize_filename::{is_sanitized, sanitize};

use crate::modules::check::missing_dependencies;
use crate::modules::config::NclConfig;
use crate::modules::scaffold::ProjectOptions;
use crate::modules::templates::{load_templates, Template};

fn prompt_project_name() -> Result<String> {
    let cwd = std::env::current_dir()?;
    
    cliclack::input("Name your project:")
        .placeholder("awesome-project")
        .validate(move |input: &String| {
            if input.is_empty() {
                Err("Please enter a name.")
            } else if !is_sanitized(input) {
                Err("Please enter a valid name.")
            } else {
                let project_path = cwd.join(input);
                if project_path.exists() && std::fs::read_dir(&project_path)
                    .map(|dir| dir.count() > 0)
                    .unwrap_or(false)
                {
                    Err("Directory is non-empty.")
                } else {
                    Ok(())
                }
            }
        })
        .interact()
        .map_err(|e| anyhow::anyhow!("Input error: {}", e))
}

fn prompt_template_selection() -> Result<Template> {
    let templates = load_templates()
        .context("Failed to load templates")?;

    if templates.is_empty() {
        return Err(anyhow::anyhow!("No templates available. Please ensure templates are installed."));
    }

    let mut selector = cliclack::select("Pick a project type:");
    
    for template in &templates {
        let comment = format!(
            "{} - {}",
            template.comment,
            template.path.to_str().unwrap_or("unknown path")
        );
        selector = selector.item(template.clone(), template.name.clone(), comment);
    }
    
    selector.interact()
        .map_err(|e| anyhow::anyhow!("Selection error: {}", e))
}

fn format_missing_dependencies(deps: &[crate::modules::common::Dependency]) -> String {
    deps.iter()
        .map(|dep| format!("{} -> install: {}", dep.name, dep.install_hint))
        .collect::<Vec<_>>()
        .join("\n")
}

fn display_missing_dependencies(template: &Template) -> Result<()> {
    let missing = missing_dependencies(template);
    if !missing.is_empty() {
        cliclack::outro_note("Missing dependencies!", &format_missing_dependencies(&missing))?;
        return Err(anyhow!("Missing dependencies error: Can't run the project without dependencies"));
    }
    Ok(())
}

pub fn run(skip_github: bool, skip_coolify: bool, skip_trello: bool) -> Result<()> {
    debug!("Starting project initialization");
    
    cliclack::clear_screen()?;
    cliclack::intro(style(" init ").on_green().black())?;

    let mut config = NclConfig::load()
        .context("Failed to load NCL configuration")?;
    
    // Override config with CLI flags
    if skip_github {
        config.skip_github = true;
    }
    if skip_coolify {
        config.skip_coolify = true;
    }
    if skip_trello {
        config.skip_trello = true;
    }
    
    let cwd = std::env::current_dir()?;
    let project_name = prompt_project_name()?;
    let template = prompt_template_selection()?;
    
    display_missing_dependencies(&template)?;

    let mut project_options = ProjectOptions {
        name: project_name.clone(),
        path: cwd.join(&project_name),
        template,
        config,
    };

    project_options.initialize_project()
        .context("Failed to initialize project")?;

    let next_steps = format!("cd ./{}\nncl run dev", sanitize(&project_options.name));
    cliclack::note("Next steps.", next_steps)?;
    cliclack::outro("Successfully initialized the project!")?;

    Ok(())
}


