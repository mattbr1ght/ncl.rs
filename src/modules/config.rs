use anyhow::{Context, Result};
use log::warn;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use toml::Value;
use toml::map::Map;

use crate::modules::common::ncl_config_dir;
use crate::modules::keyring::{Keyring, accounts};

/// GitHub configuration defaults
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct GitHubDefaults {
    /// Stored GitHub personal access token
    #[serde(default)]
    pub token: Option<String>,
    /// Default organization/owner to use (if empty, uses personal)
    #[serde(default, alias = "default_org")]
    pub owner: Option<String>,
    /// Default repository visibility
    #[serde(default, alias = "default_visibility")]
    pub repo_visibility: Option<String>,
}

/// Coolify configuration defaults (global)
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct CoolifyDefaults {
    /// Coolify API endpoint
    #[serde(default)]
    pub api_endpoint: Option<String>,
    /// Coolify API token
    #[serde(default, alias = "api_token")]
    pub token: Option<String>,
    /// Last known production environment identifier
    #[serde(default)]
    pub environment_prod_id: Option<String>,
    /// Last known development environment identifier
    #[serde(default)]
    pub environment_dev_id: Option<String>,
}

/// Project-level Coolify configuration
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ProjectCoolifyConfig {
    /// Coolify project ID (created during scaffolding)
    #[serde(default)]
    pub project_id: Option<String>,
    /// Coolify API endpoint (copied from global defaults)
    #[serde(default)]
    pub api_endpoint: Option<String>,
    /// Server ID used for this project
    #[serde(default)]
    pub server_id: Option<String>,
    /// Webhook URL for dev environment
    #[serde(default)]
    pub webhook_dev: Option<String>,
    /// Webhook URL for prod environment
    #[serde(default)]
    pub webhook_prod: Option<String>,
}

/// Project-level configuration stored in .ncl/config.toml
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct ProjectConfig {
    /// Coolify project configuration
    #[serde(default)]
    pub coolify: ProjectCoolifyConfig,
    /// GitHub repository URL
    #[serde(default)]
    pub github_repo_url: Option<String>,
    /// GitHub repository owner (user or org)
    #[serde(default)]
    pub github_repo_owner: Option<String>,
    /// GitHub repository name
    #[serde(default)]
    pub github_repo_name: Option<String>,
}

/// Global NCL configuration stored in config directory
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct NclConfig {
    /// Skip GitHub integration during project initialization
    #[serde(default)]
    pub skip_github: bool,
    /// Skip Trello integration (future feature)
    #[serde(default)]
    pub skip_trello: bool,
    /// Skip Coolify CI/CD setup (future feature)
    #[serde(default)]
    pub skip_coolify: bool,
    /// Default preferences for project initialization
    #[serde(default)]
    pub defaults: HashMap<String, String>,
    /// GitHub defaults
    #[serde(default)]
    pub github: GitHubDefaults,
    /// Coolify defaults
    #[serde(default)]
    pub coolify: CoolifyDefaults,
}

impl NclConfig {
    /// Load configuration from file, creating default if it doesn't exist
    pub fn load() -> Result<Self> {
        let config_path = config_file_path();

        if !config_path.exists() {
            let default_config = Self::default();
            default_config.save()?;
            return Ok(default_config);
        }

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let mut raw_value: Value =
            toml::from_str(&content).with_context(|| "Failed to parse config file")?;

        let migrated = migrate_legacy_fields(&mut raw_value);

        let config: NclConfig = toml::from_str(
            &toml::to_string(&raw_value).context("Failed to serialize migrated config")?,
        )
        .with_context(|| "Failed to deserialize config file")?;

        if migrated {
            config.save()?;
        }

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = config_file_path();
        std::fs::create_dir_all(config_path.parent().unwrap())?;

        let content = toml::to_string_pretty(self).context("Failed to serialize config")?;

        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }

    /// Get GitHub token from keyring or config
    pub fn get_github_token(&self) -> Result<Option<String>> {
        if let Some(token) = self.github.token.clone() {
            return Ok(Some(token));
        }

        if Keyring::is_available() {
            match Keyring::get(accounts::GITHUB_TOKEN) {
                Ok(token) => return Ok(token),
                Err(err) => {
                    warn!("Failed to read GitHub token from keyring: {}", err);
                }
            }
        }

        Ok(None)
    }

    /// Set GitHub token in both keyring and config
    pub fn set_github_token(&mut self, token: Option<String>) -> Result<()> {
        self.github.token = token.clone();

        if Keyring::is_available() {
            match token {
                Some(ref value) => {
                    if let Err(err) = Keyring::set(accounts::GITHUB_TOKEN, value) {
                        warn!("Failed to store GitHub token in keyring: {}", err);
                    }
                }
                None => {
                    if let Err(err) = Keyring::delete(accounts::GITHUB_TOKEN) {
                        warn!("Failed to clear GitHub token from keyring: {}", err);
                    }
                }
            }
        }

        self.save()
    }

    /// Get Coolify token from keyring
    pub fn get_coolify_token(&self) -> Result<Option<String>> {
        if let Some(token) = self.coolify.token.clone() {
            return Ok(Some(token));
        }

        if Keyring::is_available() {
            match Keyring::get(accounts::COOLIFY_TOKEN) {
                Ok(token) => return Ok(token),
                Err(err) => {
                    warn!("Failed to read Coolify token from keyring: {}", err);
                }
            }
        }

        Ok(None)
    }

    /// Set Coolify token in keyring
    pub fn set_coolify_token(&mut self, token: Option<String>) -> Result<()> {
        self.coolify.token = token.clone();

        if Keyring::is_available() {
            match token {
                Some(ref value) => {
                    if let Err(err) = Keyring::set(accounts::COOLIFY_TOKEN, value) {
                        warn!("Failed to store Coolify token in keyring: {}", err);
                    }
                }
                None => {
                    if let Err(err) = Keyring::delete(accounts::COOLIFY_TOKEN) {
                        warn!("Failed to clear Coolify token from keyring: {}", err);
                    }
                }
            }
        }

        self.save()
    }

    /// Get registry credentials from keyring
    pub fn get_registry_credentials(&self) -> Result<Option<(String, String)>> {
        let user = Keyring::get(accounts::REGISTRY_USER)?;
        let token = Keyring::get(accounts::REGISTRY_TOKEN)?;

        match (user, token) {
            (Some(u), Some(t)) => Ok(Some((u, t))),
            _ => Ok(None),
        }
    }

    /// Set registry credentials in keyring
    pub fn set_registry_credentials(
        &mut self,
        user: Option<String>,
        token: Option<String>,
    ) -> Result<()> {
        if let Some(ref u) = user {
            Keyring::set(accounts::REGISTRY_USER, u)?;
        } else {
            let _ = Keyring::delete(accounts::REGISTRY_USER);
        }

        if let Some(ref t) = token {
            Keyring::set(accounts::REGISTRY_TOKEN, t)?;
        } else {
            let _ = Keyring::delete(accounts::REGISTRY_TOKEN);
        }

        Ok(())
    }
}

fn config_file_path() -> PathBuf {
    ncl_config_dir().join("config.toml")
}

fn migrate_legacy_fields(value: &mut Value) -> bool {
    let mut migrated = false;

    if let Some(table) = value.as_table_mut() {
        if let Some(legacy_github_token) = table.remove("github_token") {
            if let Some(token_str) = legacy_github_token.as_str() {
                let github_entry = table
                    .entry("github")
                    .or_insert_with(|| Value::Table(Map::new()));
                if let Some(github_table) = github_entry.as_table_mut() {
                    github_table.insert("token".into(), Value::String(token_str.to_string()));
                    migrated = true;
                }
            }
        }

        if let Some(coolify_value) = table.get_mut("coolify") {
            if let Some(coolify_table) = coolify_value.as_table_mut() {
                if let Some(api_token_value) = coolify_table.remove("api_token") {
                    if coolify_table.get("token").is_none() {
                        coolify_table.insert("token".into(), api_token_value);
                    }
                    migrated = true;
                }
            }
        }
    }

    migrated
}

impl ProjectConfig {
    /// Load project configuration from .ncl/config.toml in project directory
    pub fn load(project_path: &PathBuf) -> Result<Self> {
        let config_path = project_path.join(".ncl").join("config.toml");

        if !config_path.exists() {
            return Ok(Self::default());
        }

        let content = std::fs::read_to_string(&config_path).with_context(|| {
            format!(
                "Failed to read project config file: {}",
                config_path.display()
            )
        })?;

        toml::from_str(&content).with_context(|| "Failed to parse project config file")
    }

    /// Save project configuration to .ncl/config.toml
    pub fn save(&self, project_path: &PathBuf) -> Result<()> {
        let config_dir = project_path.join(".ncl");
        std::fs::create_dir_all(&config_dir).with_context(|| {
            format!("Failed to create .ncl directory: {}", config_dir.display())
        })?;

        let config_path = config_dir.join("config.toml");

        let content = toml::to_string_pretty(self).context("Failed to serialize project config")?;

        std::fs::write(&config_path, content).with_context(|| {
            format!(
                "Failed to write project config file: {}",
                config_path.display()
            )
        })?;

        Ok(())
    }
}
