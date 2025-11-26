use anyhow::{Context, Result};
use log::{debug, warn};
use std::path::PathBuf;

use crate::modules::config::{NclConfig, ProjectConfig};
use crate::modules::coolify::CoolifyClient;
use crate::modules::github;

/// Removes a project (GitHub repo, Coolify project, keyring entries, config)
pub fn remove_project(project_path: &PathBuf) -> Result<()> {
    debug!("Removing project at {}", project_path.display());

    // Load project config
    let project_config =
        ProjectConfig::load(project_path).context("Failed to load project configuration")?;

    let global_config = NclConfig::load().context("Failed to load global configuration")?;

    let mut github_deleted = false;
    let mut coolify_deleted = false;

    // Delete GitHub repository
    if let (Some(owner), Some(repo)) = (
        &project_config.github_repo_owner,
        &project_config.github_repo_name,
    ) {
        if let Ok(Some(token)) = global_config.get_github_token() {
            cliclack::log::info(&format!("Deleting GitHub repository {}/{}", owner, repo))?;
            match github::delete_repository(&token, owner, repo) {
                Ok(()) => {
                    cliclack::log::success("GitHub repository deleted")?;
                    github_deleted = true;
                }
                Err(e) => {
                    warn!("Failed to delete GitHub repository: {}", e);
                    cliclack::log::warning(&format!("Failed to delete GitHub repository: {}", e))?;
                }
            }
        } else {
            warn!("GitHub token not available, skipping repository deletion");
        }
    } else {
        github_deleted = true; // No repo to delete
    }

    // Delete Coolify project
    if let Some(project_id) = &project_config.coolify.project_id {
        if let Some(endpoint) = project_config.coolify.api_endpoint.as_deref() {
            if let Ok(Some(token)) = global_config.get_coolify_token() {
                cliclack::log::info("Deleting Coolify project...")?;
                let client = CoolifyClient::new(endpoint.to_string(), token);
                match client.delete_project(project_id) {
                    Ok(()) => {
                        cliclack::log::success("Coolify project deleted")?;
                        coolify_deleted = true;
                    }
                    Err(e) => {
                        warn!("Failed to delete Coolify project: {}", e);
                        cliclack::log::warning(&format!(
                            "Failed to delete Coolify project: {}",
                            e
                        ))?;
                    }
                }
            } else {
                warn!("Coolify token not available, skipping project deletion");
            }
        } else {
            warn!("Coolify endpoint not configured, skipping project deletion");
        }
    } else {
        coolify_deleted = true; // No project to delete
    }

    // Only delete .ncl/config.toml if both GitHub and Coolify have been successfully removed
    if github_deleted && coolify_deleted {
        let config_path = project_path.join(".ncl").join("config.toml");
        if config_path.exists() {
            std::fs::remove_file(&config_path).context("Failed to delete project config")?;
            cliclack::log::success("Project configuration deleted")?;
        }
    } else {
        warn!("Keeping project configuration because some resources could not be deleted");
        cliclack::log::warning(
            "Project configuration kept because some resources could not be deleted",
        )?;
    }

    cliclack::outro("Project cleanup completed")?;
    Ok(())
}
