use anyhow::{Context, Result};
use log::debug;
use regex::Regex;
use serde::Serialize;
use std::collections::HashMap;

use crate::modules::config::NclConfig;
use crate::modules::templates::{run_hook, Template, load_universal_base};
use crate::modules::common::Installable;
use crate::modules::permissions::fix_file_ownership;

#[derive(Serialize)]
pub struct ProjectOptions {
    pub name: String,
    #[serde(skip_serializing)]
    pub path: std::path::PathBuf,
    #[serde(skip_serializing)]
    pub template: Template,
    #[serde(skip_serializing)]
    pub config: NclConfig,
}

impl ProjectOptions {
    pub fn vars(&self) -> HashMap<&str, &str> {
        let mut map = HashMap::new();
        map.insert("project_name", self.name.as_str());
        map.insert("path", self.path.to_str().unwrap());
        map
    }

    pub fn initialize_project(&mut self) -> Result<()> {
        debug!("Initializing project: {}", self.name);
        
        self.create_project_directory()?;
        self.install_universal_base()?;
        self.install_selected_template()?;
        self.save_project_options()?;
        
        // Fix ownership after template installation (before hooks)
        fix_file_ownership(&self.path)
            .context("Failed to fix file ownership after template installation")?;
        
        self.run_post_install_hook()?;
        
        // Fix ownership again after hooks (Docker might create root-owned files)
        // This is best-effort - if files are root-owned, user may need to run chown manually
        if let Err(e) = fix_file_ownership(&self.path) {
            debug!("Could not fix all file ownership: {}", e);
            cliclack::log::warning("Some files may be root-owned. If you encounter permission issues, run:")?;
            cliclack::log::warning(&format!("  sudo chown -R $USER:$USER {}", self.path.display()))?;
        }
        
        let repo = self.initialize_git_repository()?;
        
        if !self.config.skip_github {
            self.setup_github_integration(repo)?;
        } else {
            debug!("Skipping GitHub integration");
        }
        
        Ok(())
    }

    fn create_project_directory(&self) -> Result<()> {
        if !self.path.exists() {
            std::fs::create_dir_all(&self.path)
                .with_context(|| format!("Failed to create project directory: {}", self.path.display()))?;
        }
        Ok(())
    }

    fn install_universal_base(&self) -> Result<()> {
        if let Some(base_template) = load_universal_base()? {
            debug!("Installing universal base template");
            base_template.install(self)
                .context("Failed to install universal base template")?;
        } else {
            debug!("No universal base template found");
        }
        Ok(())
    }

    fn install_selected_template(&self) -> Result<()> {
        debug!("Installing template: {}", self.template.name);
        self.template.install(self)
            .with_context(|| format!("Failed to install template '{}'", self.template.name))
    }

    fn save_project_options(&self) -> Result<()> {
        let toml_content = toml::to_string(self)
            .context("Failed to serialize project options")?;
        std::fs::write(self.path.join("ncl_project_options.toml"), toml_content)
            .context("Failed to write project options file")?;
        Ok(())
    }

    fn run_post_install_hook(&self) -> Result<()> {
        let hook = self.template.get_post_install_hook();
        if !hook.is_empty() {
            debug!("Running post-install hook with {} commands", hook.len());
            run_hook(hook, &self.path)
                .context("Failed to run post-install hook")?;
        }
        Ok(())
    }

    fn initialize_git_repository(&self) -> Result<Option<git2::Repository>> {
        if self.path.join(".git").exists() {
            debug!("Git repository already exists");
            return Ok(None);
        }

        debug!("Initializing git repository");
        let repo = git2::Repository::init(&self.path)
            .with_context(|| format!("Failed to initialize git repository at {}", self.path.display()))?;
        
        Ok(Some(repo))
    }

    fn setup_github_integration(&mut self, repo: Option<git2::Repository>) -> Result<()> {
        debug!("Setting up GitHub integration");
        
        let token = self.get_github_token()?;
        if token.is_none() || token.as_ref().unwrap().is_empty() {
            debug!("No GitHub token available, skipping repository creation");
            return Ok(());
        }

        let token = token.unwrap();
        self.create_remote_repository(&token, repo)
            .context("Failed to create GitHub repository")
    }

    fn get_github_token(&mut self) -> Result<Option<String>> {
        // Try to get from config
        if let Ok(Some(token)) = self.config.get_github_token() {
            if !token.is_empty() {
                return Ok(Some(token));
            }
        }

        // Ask user if they want to provide a token
        let should_ask = cliclack::confirm("Would you like to set up GitHub integration?")
            .initial_value(true)
            .interact()
            .unwrap_or(false);

        if !should_ask {
            return Ok(None);
        }

        let token = ask_for_github_token();
        if !token.is_empty() {
            self.config.set_github_token(Some(token.clone()))?;
            Ok(Some(token))
        } else {
            Ok(None)
        }
    }

    fn create_remote_repository(
        &self,
        token: &str,
        repo: Option<git2::Repository>,
    ) -> Result<()> {
        let client = reqwest::blocking::Client::new();
        let response = client
            .post("https://api.github.com/user/repos")
            .header("User-Agent", "NCL-CLI")
            .bearer_auth(token)
            .json(&serde_json::json!({
                "name": self.name,
                "private": true
            }))
            .send()
            .context("Failed to send request to GitHub API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            cliclack::log::error(&format!("Failed to create GitHub repository: {}", error_text))?;
            return Ok(());
        }

        cliclack::log::info("Successfully created remote repository")?;

        #[derive(serde::Deserialize)]
        struct GitHubResponse {
            clone_url: String,
        }

        let github_response: GitHubResponse = response.json()
            .context("Failed to parse GitHub API response")?;

        if let Some(repo) = repo {
            repo.remote("origin", &github_response.clone_url)
                .context("Failed to add remote origin to git repository")?;
            debug!("Added GitHub remote: {}", github_response.clone_url);
        }

        Ok(())
    }
}

fn ask_for_github_token() -> String {
    cliclack::input("Please provide a GitHub access token (optional - leave empty to skip):")
        .required(false)
        .validate_interactively(|input: &String| {
            if input.is_empty() {
                Ok(())
            } else if Regex::new(r"^ghp_[a-zA-Z0-9]{36}$")
                .unwrap()
                .is_match(input)
            {
                Ok(())
            } else {
                Err("Not a valid personal access token")
            }
        })
        .interact()
        .unwrap_or_default()
}
