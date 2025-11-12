use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::modules::common::ncl_config_dir;

/// Global NCL configuration stored in config directory
#[derive(Serialize, Deserialize, Debug, Clone, Default)]
pub struct NclConfig {
    /// GitHub personal access token (also stored in keyring)
    pub github_token: Option<String>,
    /// Skip GitHub integration during project initialization
    pub skip_github: bool,
    /// Skip Trello integration (future feature)
    pub skip_trello: bool,
    /// Skip Coolify CI/CD setup (future feature)
    pub skip_coolify: bool,
    /// Default preferences for project initialization
    pub defaults: HashMap<String, String>,
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
        
        toml::from_str(&content)
            .with_context(|| "Failed to parse config file")
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let config_path = config_file_path();
        std::fs::create_dir_all(config_path.parent().unwrap())?;
        
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;
        
        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;
        
        Ok(())
    }

    /// Get GitHub token from config or keyring
    pub fn get_github_token(&self) -> Result<Option<String>> {
        // First try keyring
        if let Ok(token) = get_token_from_keyring() {
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }

        // Fallback to config file
        Ok(self.github_token.clone())
    }

    /// Set GitHub token in both keyring and config
    pub fn set_github_token(&mut self, token: Option<String>) -> Result<()> {
        if let Some(ref token) = token {
            store_token_in_keyring(token)?;
        }
        self.github_token = token;
        self.save()
    }
}

fn config_file_path() -> PathBuf {
    ncl_config_dir().join("config.toml")
}

fn get_token_from_keyring() -> Result<String> {
    use keyring::Entry;
    
    let service = "ncl-cli";
    let user = whoami::username();
    let entry = Entry::new(service, &user)
        .context("Failed to access keyring")?;
    
    entry.get_password()
        .map_err(|e| anyhow::anyhow!("Keyring error: {}", e))
}

fn store_token_in_keyring(token: &str) -> Result<()> {
    use keyring::Entry;
    
    let service = "ncl-cli";
    let user = whoami::username();
    let entry = Entry::new(service, &user)
        .context("Failed to access keyring")?;
    
    entry.set_password(token)
        .map_err(|e| anyhow::anyhow!("Failed to store token in keyring: {}", e))
}

