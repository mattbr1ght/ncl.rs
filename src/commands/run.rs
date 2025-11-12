use anyhow::{Context, Result};

use crate::modules::common::is_valid_project_path;
use crate::modules::templates::{run_hook, Template};

fn load_template_from_current_dir() -> Result<Template> {
    let cwd = std::env::current_dir()?;
    let meta = std::fs::read_to_string(cwd.join("template.toml"))
        .context("Failed to read template.toml")?;
    let mut template: Template = toml::from_str(&meta)
        .context("Failed to parse template.toml")?;
    template.path = cwd;
    Ok(template)
}

fn list_available_jobs(jobs: &std::collections::HashMap<String, Vec<String>>) {
    for (script, commands) in jobs {
        println!("{}", script);
        for command in commands {
            println!("    {}", command);
        }
    }
}

pub fn run(job: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    
    if !is_valid_project_path(&cwd) {
        cliclack::outro("You are not in a valid project directory")?;
        return Ok(());
    }

    let template = load_template_from_current_dir()?;
    let jobs = match template.jobs {
        Some(jobs) => jobs,
        None => {
            cliclack::outro("No jobs defined in template.toml")?;
            return Ok(());
        }
    };

    match job {
        None => {
            list_available_jobs(&jobs);
            Ok(())
        }
        Some(job_name) => {
            match jobs.get(&job_name) {
                Some(commands) => {
                    run_hook(commands, &cwd)?;
                    cliclack::outro("")?;
                    Ok(())
                }
                None => {
                    cliclack::outro("The job does not exist")?;
                    Ok(())
                }
            }
        }
    }
}
