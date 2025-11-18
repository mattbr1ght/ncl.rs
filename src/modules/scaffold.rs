use anyhow::{Context, Result};
use log::debug;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::modules::config::{NclConfig, ProjectConfig};
use crate::modules::templates::{run_hook, Template, load_universal_base};
use crate::modules::common::Installable;
use crate::modules::permissions::fix_file_ownership;
use crate::modules::github::{self, CreateRepoRequest};
use crate::modules::git;
use crate::modules::coolify::{CoolifyClient, prompt_coolify_config, prompt_coolify_setup};
use crate::modules::keyring::{Keyring, accounts};

/// Rollback state for error handling
#[derive(Default)]
struct RollbackState {
    coolify_project_id: Option<String>,
    github_repo_owner: Option<String>,
    github_repo_name: Option<String>,
}

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
        
        // Copy CI/CD workflow from template if it exists
        self.copy_cicd_workflow()?;
        
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
        
        // Initialize project config
        let mut project_config = ProjectConfig::default();
        
        // Track what we've created for rollback
        let mut rollback_state = RollbackState::default();
        
        // Setup Coolify integration (optional, prompt user)
        let setup_coolify = if self.config.skip_coolify {
            false
        } else {
            prompt_coolify_setup()
                .context("Failed to prompt for Coolify setup")?
        };
        
        if setup_coolify {
            match self.setup_coolify_integration(&mut project_config) {
                Ok(()) => {
                    rollback_state.coolify_project_id = project_config.coolify.project_id.clone();
                }
                Err(e) => {
                    self.rollback(&rollback_state)?;
                    return Err(e).context("Failed to set up Coolify integration");
                }
            }
        } else {
            debug!("Skipping Coolify integration");
        }
        
        // Initialize git repository and make first commit on dev branch
        let mut repo = match self.initialize_git_and_commit() {
            Ok(r) => r,
            Err(e) => {
                self.rollback(&rollback_state)?;
                return Err(e).context("Failed to initialize git repository");
            }
        };
        
        // Setup GitHub integration (create repo, set dev as default, add protection)
        if !self.config.skip_github {
            match self.setup_github_integration(&mut repo, &mut project_config, setup_coolify) {
                Ok(()) => {
                    rollback_state.github_repo_owner = project_config.github_repo_owner.clone();
                    rollback_state.github_repo_name = project_config.github_repo_name.clone();
                }
                Err(e) => {
                    self.rollback(&rollback_state)?;
                    return Err(e).context("Failed to set up GitHub integration");
                }
            }
        } else {
            debug!("Skipping GitHub integration");
        }
        
        // Save project config to .ncl/config.toml
        if let Err(e) = project_config.save(&self.path) {
            self.rollback(&rollback_state)?;
            return Err(e).context("Failed to save project configuration");
        }
        
        // Make another commit with .ncl/config.toml
        if let Err(e) = self.commit_project_config(&repo) {
            self.rollback(&rollback_state)?;
            return Err(e).context("Failed to commit project configuration");
        }
        
        // Push all changes
        if !self.config.skip_github {
            if let Err(e) = self.push_all_branches(&repo) {
                self.rollback(&rollback_state)?;
                return Err(e).context("Failed to push branches");
            }
        }
        
        Ok(())
    }
    
    /// Rollback created resources on error
    fn rollback(&self, state: &RollbackState) -> Result<()> {
        debug!("Rolling back created resources");
        
        // Delete GitHub repo if created
        if let (Some(owner), Some(repo)) = (&state.github_repo_owner, &state.github_repo_name) {
            if let Ok(Some(token)) = self.config.get_github_token() {
                let _ = github::delete_repository(&token, owner, repo);
            }
        }
        
        // Delete Coolify project if created
        if let Some(project_id) = &state.coolify_project_id {
            if let Some(endpoint) = self.config.coolify.api_endpoint.as_deref() {
                if let Ok(Some(token)) = self.config.get_coolify_token() {
                    let client = CoolifyClient::new(endpoint.to_string(), token);
                    let _ = client.delete_project(project_id);
                }
            }
        }
        
        // Clean up keyring entries (project-specific)
        let project_key = format!("{}_{}", self.name, accounts::COOLIFY_TOKEN);
        let _ = Keyring::delete(&project_key);
        
        // Delete .ncl/config.toml if it exists
        let config_path = self.path.join(".ncl").join("config.toml");
        let _ = std::fs::remove_file(&config_path);
        
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

    fn copy_cicd_workflow(&self) -> Result<()> {
        let template_workflow = self.template.path.join(".github").join("workflows").join("ci.yml");
        
        if !template_workflow.exists() {
            debug!("No CI/CD workflow found in template, skipping");
            return Ok(());
        }
        
        let target_workflow_dir = self.path.join(".github").join("workflows");
        std::fs::create_dir_all(&target_workflow_dir)
            .context("Failed to create .github/workflows directory")?;
        
        let target_workflow = target_workflow_dir.join("ci.yml");
        std::fs::copy(&template_workflow, &target_workflow)
            .with_context(|| format!("Failed to copy CI workflow from {} to {}", 
                template_workflow.display(), target_workflow.display()))?;
        
        debug!("Copied CI/CD workflow from template");
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

    fn initialize_git_and_commit(&self) -> Result<git2::Repository> {
        debug!("Initializing git repository and making first commit");
        
        let repo = git::initialize_and_commit(
            &self.path,
            "Initial commit"
        )
        .context("Failed to initialize git repository and create initial commit")?;
        
        Ok(repo)
    }

    fn setup_github_integration(&mut self, repo: &mut git2::Repository, project_config: &mut ProjectConfig, has_coolify: bool) -> Result<()> {
        debug!("Setting up GitHub integration");
        
        let token = self.get_github_token()?;
        if token.is_none() || token.as_ref().unwrap().is_empty() {
            debug!("No GitHub token available, skipping repository creation");
            return Ok(());
        }

        let token = token.unwrap();
        
        // Prompt for repository location (personal or organization)
        let org = self.prompt_repository_location(&token)?;
        
        // Determine owner (org or user)
        let owner = if let Some(ref org_name) = org {
            org_name.clone()
        } else {
            // Get current user
            let client = reqwest::blocking::Client::new();
            let response = client
                .get("https://api.github.com/user")
                .header("User-Agent", "NCL-CLI")
                .header("Accept", "application/vnd.github.v3+json")
                .bearer_auth(&token)
                .send()
                .context("Failed to get GitHub user")?;
            
            if !response.status().is_success() {
                return Err(anyhow::anyhow!("Failed to get GitHub user info"));
            }
            
            #[derive(Deserialize)]
            struct GitHubUser {
                login: String,
            }
            
            let user: GitHubUser = response.json()
                .context("Failed to parse GitHub user response")?;
            user.login
        };
        
        // Create remote repository with dev as default branch
        let is_private = self.config.github.default_visibility.as_deref().unwrap_or("private") == "private";
        let repo_request = CreateRepoRequest {
            name: self.name.clone(),
            private: is_private,
            description: None,
            default_branch: Some("dev".to_string()),
        };
        
        let repo_response = github::create_repository(&token, org.as_deref(), &repo_request)?;
        
        // Store in project config
        project_config.github_repo_url = Some(repo_response.clone_url.clone());
        project_config.github_repo_owner = Some(owner.clone());
        project_config.github_repo_name = Some(self.name.clone());
        
        // Add remote and push dev branch
        self.setup_remote_and_push(&repo, &repo_response.clone_url, "dev", true)?;
        
        // Create prod branch from dev
        git::create_branch(&repo, "prod")?;
        
        // Push prod branch
        git::push_to_remote(&repo, "origin", "prod", true)?;
        
        // Set dev as default branch on GitHub
        github::set_default_branch(&token, &owner, &self.name, "dev")?;
        
        // // Add branch protection rules
        // // Protect prod: require PR, require status checks, no direct pushes
        // github::add_branch_protection(&token, &owner, &self.name, "prod", true, true, true)?;
        //
        // // Protect dev: optional stricter rules (require status checks, but PR optional)
        // github::add_branch_protection(&token, &owner, &self.name, "dev", false, true, false)?;
        
        // Enable GitHub Actions
        github::enable_github_actions(&token, &owner, &self.name)?;
        
        // Create deployment environments
        github::create_deployment_environment(&token, &owner, &self.name, "development")?;
        github::create_deployment_environment(&token, &owner, &self.name, "production")?;
        
        // Set up GitHub secrets if Coolify is configured
        if has_coolify {
            if let Some(coolify_token) = project_config.coolify.api_endpoint.as_ref()
                .and_then(|_| self.config.get_coolify_token().ok().flatten()) {
                github::create_or_update_secret(&token, &owner, &self.name, "COOLIFY_TOKEN", &coolify_token)?;
            }
            
            if let Some(webhook_dev) = &project_config.coolify.webhook_dev {
                github::create_or_update_secret(&token, &owner, &self.name, "COOLIFY_WEBHOOK_DEV", webhook_dev)?;
            }
            
            if let Some(webhook_prod) = &project_config.coolify.webhook_prod {
                github::create_or_update_secret(&token, &owner, &self.name, "COOLIFY_WEBHOOK_PROD", webhook_prod)?;
            }
        }
        
        // Set up registry secrets if configured
        if let Ok(Some((registry_user, registry_token))) = self.config.get_registry_credentials() {
            github::create_or_update_secret(&token, &owner, &self.name, "REGISTRY_USER", &registry_user)?;
            github::create_or_update_secret(&token, &owner, &self.name, "REGISTRY_TOKEN", &registry_token)?;
        }
        
        // Optional branch protection
        let enable_protection = cliclack::confirm("Would you like to enable branch protection rules?")
            .initial_value(false)
            .interact()
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?;
        
        if enable_protection {
            // Check if private repo (warn about GitHub Pro/Team requirement)
            if is_private {
                cliclack::log::warning("Note: Branch protection for private repos requires GitHub Pro or Team plan")?;
            }
            
            // Protect prod: require PR, require status checks, no direct pushes
            github::add_branch_protection(&token, &owner, &self.name, "prod", true, true, true)?;
            
            // Protect dev: require status checks, PR optional
            github::add_branch_protection(&token, &owner, &self.name, "dev", false, true, false)?;
            
            cliclack::log::info("Branch protection rules enabled")?;
        }
        
        cliclack::log::info("GitHub repository configured with dev as default branch")?;
        
        Ok(())
    }
    
    fn prompt_repository_location(&self, token: &str) -> Result<Option<String>> {
        // Ask user where to create the repo
        let mut selector = cliclack::select("Where should the repository be created?");
        selector = selector.item("personal", "Personal account", "");
        selector = selector.item("org", "Organization", "");
        
        let location = selector.interact()
            .map_err(|e| anyhow::anyhow!("Selection error: {}", e))?;

        if location == "personal" {
            // Personal account
            return Ok(None);
        }

        // Organization selected - fetch and prompt for selection
        let orgs = github::fetch_organizations(token)
            .context("Failed to fetch organizations")?;

        if orgs.is_empty() {
            cliclack::log::warning("No organizations found. Creating repository in personal account.")?;
            return Ok(None);
        }

        let selected_org = github::prompt_organization_selection(&orgs)?;
        Ok(selected_org)
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

    fn setup_remote_and_push(
        &self,
        repo: &git2::Repository,
        clone_url: &str,
        branch: &str,
        set_upstream: bool,
    ) -> Result<()> {
        debug!("Setting up remote and pushing {} branch", branch);
        
        // Embed token in URL for authentication
        let token = self.config.get_github_token()?
            .ok_or_else(|| anyhow::anyhow!("GitHub token required for push"))?;
        
        // Convert https://github.com/user/repo.git to https://token@github.com/user/repo.git
        let authenticated_url = if clone_url.starts_with("https://") {
            clone_url.replace("https://", &format!("https://{}@", token))
        } else {
            clone_url.to_string()
        };
        
        // Add remote with authenticated URL
        repo.remote("origin", &authenticated_url)
            .context("Failed to add remote origin to git repository")?;
        
        debug!("Added GitHub remote: {}", clone_url);
        
        // Push branch
        git::push_to_remote(repo, "origin", branch, set_upstream)
            .context("Failed to push branch")?;
        
        cliclack::log::info(&format!("Pushed {} branch to remote", branch))?;
        
        Ok(())
    }
    
    fn setup_coolify_integration(&self, project_config: &mut ProjectConfig) -> Result<()> {
        debug!("Setting up Coolify integration");
        
        // Prompt for Coolify configuration
        let (api_endpoint, api_token, server_id) = prompt_coolify_config(
            self.config.coolify.api_endpoint.as_deref(),
            self.config.coolify.api_token.as_deref(),
            self.config.coolify.default_server_id.as_deref(),
        )
        .context("Failed to get Coolify configuration")?;
        
        // Store token in keyring
        let mut global_config = self.config.clone();
        global_config.set_coolify_token(Some(api_token.clone()))?;
        
        // Store in project config
        project_config.coolify.api_endpoint = Some(api_endpoint.clone());
        project_config.coolify.server_id = server_id.clone();
        
        // Create Coolify client and project
        let client = CoolifyClient::new(api_endpoint.clone(), api_token.clone());
        
        // Normalize project name (lowercase, hyphens instead of underscores)
        let coolify_project_name = self.name.to_lowercase().replace('_', "-");
        
        cliclack::log::info("Creating Coolify project...")?;
        
        let project_id = client.create_project(&coolify_project_name)
            .context("Failed to create Coolify project")?;
        
        // Store project ID in config
        project_config.coolify.project_id = Some(project_id.clone());
        
        // Create dev environment
        cliclack::log::info("Creating dev environment...")?;
        let _dev_env_id = client.create_environment(&project_id, "dev")
            .context("Failed to create dev environment")?;
        
        // Get dev webhook
        let webhook_dev = client.get_environment_webhook(&project_id, "dev")
            .context("Failed to get dev webhook")?;
        project_config.coolify.webhook_dev = webhook_dev.clone();
        
        // Create prod environment (rename from default "production" if needed)
        cliclack::log::info("Creating prod environment...")?;
        let _prod_env_id = client.create_environment(&project_id, "prod")
            .context("Failed to create prod environment")?;
        
        // Get prod webhook
        let webhook_prod = client.get_environment_webhook(&project_id, "prod")
            .context("Failed to get prod webhook")?;
        project_config.coolify.webhook_prod = webhook_prod.clone();
        
        // Update global config with defaults for future use
        global_config.coolify.api_endpoint = Some(api_endpoint);
        if let Some(server_id) = server_id {
            global_config.coolify.default_server_id = Some(server_id);
        }
        global_config.save()?;
        
        cliclack::log::info(&format!("Successfully created Coolify project: {} with dev and prod environments", project_id))?;
        debug!("Coolify project config: {:?}", &project_config.coolify);
        debug!("Coolify global config: {:?}", &project_config.coolify);
        
        Ok(())
    }
    
    fn commit_project_config(&self, repo: &git2::Repository) -> Result<()> {
        debug!("Committing project configuration");
        
        let signature = git::get_signature(repo)?;
        let mut index = repo.index()
            .context("Failed to get repository index")?;
        
        // Add .ncl/config.toml
        let config_path = self.path.join(".ncl").join("config.toml");
        if config_path.exists() {
            index.add_path(&std::path::Path::new(".ncl/config.toml"))
                .context("Failed to add config to index")?;
        }
        
        index.write()
            .context("Failed to write index")?;
        
        let tree_id = index.write_tree()
            .context("Failed to write tree")?;
        
        let tree = repo.find_tree(tree_id)
            .context("Failed to find tree")?;
        
        let head = repo.head()
            .context("Failed to get HEAD")?;
        
        let head_commit = head.target()
            .and_then(|oid| repo.find_commit(oid).ok())
            .context("Failed to find HEAD commit")?;
        
        repo.commit(
            Some("refs/heads/dev"),
            &signature,
            &signature,
            "Add project configuration",
            &tree,
            &[&head_commit],
        )
        .context("Failed to create commit")?;
        
        drop(tree);
        debug!("Committed project configuration");
        Ok(())
    }
    
    fn push_all_branches(&self, repo: &git2::Repository) -> Result<()> {
        debug!("Pushing all branches to remote");
        
        // Push dev branch (with upstream)
        git::push_to_remote(repo, "origin", "dev", true)
            .context("Failed to push dev branch")?;
        
        // Push prod branch (with upstream)
        git::push_to_remote(repo, "origin", "prod", true)
            .context("Failed to push prod branch")?;
        
        cliclack::log::info("Pushed all branches to remote")?;
        
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
