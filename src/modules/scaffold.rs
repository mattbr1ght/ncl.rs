use anyhow::{Context, Result};
use log::debug;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::modules::common::Installable;
use crate::modules::config::{NclConfig, ProjectConfig};
use crate::modules::coolify::{
    CoolifyClient, prompt_coolify_config, prompt_coolify_setup,
    prompt_destination_selection, prompt_github_app_selection, prompt_server_selection,
};
use crate::modules::git;
use crate::modules::github::{self, CreateRepoRequest};
use crate::modules::permissions::fix_file_ownership;
use crate::modules::templates::{Template, load_universal_base, run_hook};

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
            cliclack::log::warning(
                "Some files may be root-owned. If you encounter permission issues, run:",
            )?;
            cliclack::log::warning(&format!(
                "  sudo chown -R $USER:$USER {}",
                self.path.display()
            ))?;
        }

        // Initialize project config
        let mut project_config = ProjectConfig::default();

        // Track what we've created for rollback
        let mut rollback_state = RollbackState::default();

        // Setup Coolify integration (optional, prompt user)
        let setup_coolify = if self.config.skip_coolify {
            false
        } else {
            prompt_coolify_setup().context("Failed to prompt for Coolify setup")?
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
            match self.setup_github_integration(&mut repo, &mut project_config) {
                Ok(()) => {
                    rollback_state.github_repo_owner = project_config.github_repo_owner.clone();
                    rollback_state.github_repo_name = project_config.github_repo_name.clone();

                    // Setup GitHub app resources in Coolify if Coolify is configured
                    if setup_coolify {
                        if let (Some(owner), Some(repo_name)) = (
                            &project_config.github_repo_owner,
                            &project_config.github_repo_name,
                        ) {
                            if let Err(e) = self.setup_coolify_github_resources(
                                &project_config,
                                owner,
                                repo_name,
                            ) {
                                log::warn!("Failed to setup Coolify GitHub resources: {}", e);
                                cliclack::log::warning(&format!(
                                    "Failed to setup Coolify GitHub resources: {}",
                                    e
                                ))?;
                            }
                        }
                    }
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

        // Delete .ncl/config.toml if it exists
        let config_path = self.path.join(".ncl").join("config.toml");
        let _ = std::fs::remove_file(&config_path);

        Ok(())
    }

    fn create_project_directory(&self) -> Result<()> {
        if !self.path.exists() {
            std::fs::create_dir_all(&self.path).with_context(|| {
                format!(
                    "Failed to create project directory: {}",
                    self.path.display()
                )
            })?;
        }
        Ok(())
    }

    fn install_universal_base(&self) -> Result<()> {
        if let Some(base_template) = load_universal_base()? {
            debug!("Installing universal base template");
            base_template
                .install(self)
                .context("Failed to install universal base template")?;
        } else {
            debug!("No universal base template found");
        }
        Ok(())
    }

    fn install_selected_template(&self) -> Result<()> {
        debug!("Installing template: {}", self.template.name);
        self.template
            .install(self)
            .with_context(|| format!("Failed to install template '{}'", self.template.name))
    }

    fn copy_cicd_workflow(&self) -> Result<()> {
        let template_workflow = self
            .template
            .path
            .join(".github")
            .join("workflows")
            .join("ci.yml");

        if !template_workflow.exists() {
            debug!("No CI/CD workflow found in template, skipping");
            return Ok(());
        }

        let target_workflow_dir = self.path.join(".github").join("workflows");
        std::fs::create_dir_all(&target_workflow_dir)
            .context("Failed to create .github/workflows directory")?;

        let target_workflow = target_workflow_dir.join("ci.yml");
        std::fs::copy(&template_workflow, &target_workflow).with_context(|| {
            format!(
                "Failed to copy CI workflow from {} to {}",
                template_workflow.display(),
                target_workflow.display()
            )
        })?;

        debug!("Copied CI/CD workflow from template");
        Ok(())
    }

    fn run_post_install_hook(&self) -> Result<()> {
        let hook = self.template.get_post_install_hook();
        if !hook.is_empty() {
            debug!("Running post-install hook with {} commands", hook.len());
            run_hook(hook, &self.path).context("Failed to run post-install hook")?;
        }
        Ok(())
    }

    fn initialize_git_and_commit(&self) -> Result<git2::Repository> {
        debug!("Initializing git repository and making first commit");

        let repo = git::initialize_and_commit(&self.path, "Initial commit")
            .context("Failed to initialize git repository and create initial commit")?;

        Ok(repo)
    }

    fn setup_github_integration(
        &mut self,
        repo: &mut git2::Repository,
        project_config: &mut ProjectConfig,
    ) -> Result<()> {
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

            let user: GitHubUser = response
                .json()
                .context("Failed to parse GitHub user response")?;
            user.login
        };

        // Create remote repository with dev as default branch
        let is_private = self
            .config
            .github
            .repo_visibility
            .as_deref()
            .unwrap_or("private")
            == "private";
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

        // Enable GitHub Actions
        github::enable_github_actions(&token, &owner, &self.name)?;

        // Optional branch protection
        let enable_protection =
            cliclack::confirm("Would you like to enable branch protection rules?")
                .initial_value(false)
                .interact()
                .map_err(|e| anyhow::anyhow!("Input error: {}", e))?;

        if enable_protection {
            // Check if private repo (warn about GitHub Pro/Team requirement)
            if is_private {
                cliclack::log::warning(
                    "Note: Branch protection for private repos requires GitHub Pro or Team plan",
                )?;
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

        let location = selector
            .interact()
            .map_err(|e| anyhow::anyhow!("Selection error: {}", e))?;

        if location == "personal" {
            // Personal account
            return Ok(None);
        }

        // Organization selected - fetch and prompt for selection
        let orgs = github::fetch_organizations(token).context("Failed to fetch organizations")?;

        if orgs.is_empty() {
            cliclack::log::warning(
                "No organizations found. Creating repository in personal account.",
            )?;
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
        let token = self
            .config
            .get_github_token()?
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

    fn setup_coolify_integration(&mut self, project_config: &mut ProjectConfig) -> Result<()> {
        debug!("Setting up Coolify integration");

        // Prompt for Coolify configuration (with validation)
        let (api_endpoint, api_token, server_id) = self
            .prompt_and_validate_coolify_settings()
            .context("Failed to collect Coolify configuration")?;

        // Store in project config
        project_config.coolify.api_endpoint = Some(api_endpoint.clone());
        project_config.coolify.server_id = server_id.clone();

        // Create Coolify client and project
        let client = CoolifyClient::new(api_endpoint.clone(), api_token.clone());

        // Normalize project name (lowercase, hyphens instead of underscores)
        let coolify_project_name = self.name.to_lowercase().replace('_', "-");

        cliclack::log::info("Creating Coolify project...")?;

        let project_id = client
            .create_project(&coolify_project_name)
            .context("Failed to create Coolify project")?;

        // Store project ID in config
        project_config.coolify.project_id = Some(project_id.clone());

        // Delete default "production" environment if it exists
        cliclack::log::info("Checking for default production environment...")?;
        match client.list_environments(&project_id) {
            Ok(environments) => {
                for env in environments {
                    // Check if this is the default "production" environment
                    if env.name.as_deref() == Some("production") {
                        if let Some(env_id) = env.id.clone().or_else(|| env.uuid.clone()) {
                            cliclack::log::info("Removing default production environment...")?;
                            if let Err(e) = client.delete_environment(&project_id, &env_id) {
                                log::warn!(
                                    "Failed to delete default production environment: {}",
                                    e
                                );
                            } else {
                                debug!("Deleted default production environment: {}", env_id);
                                cliclack::log::success("Removed default production environment")?;
                            }
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "Failed to list environments to check for default production: {}",
                    e
                );
            }
        }

        // Create dev environment
        cliclack::log::info("Creating dev environment...")?;
        let dev_env_id = client
            .create_environment(&project_id, "dev")
            .context("Failed to create dev environment")?;

        // Get dev webhook
        let webhook_dev = client
            .get_environment_webhook(&project_id, "dev")
            .context("Failed to get dev webhook")?;
        project_config.coolify.webhook_dev = webhook_dev.clone();

        // Create prod environment
        cliclack::log::info("Creating prod environment...")?;
        let prod_env_id = client
            .create_environment(&project_id, "prod")
            .context("Failed to create prod environment")?;

        // Get prod webhook
        let webhook_prod = client
            .get_environment_webhook(&project_id, "prod")
            .context("Failed to get prod webhook")?;
        project_config.coolify.webhook_prod = webhook_prod.clone();

        // Track last created environment IDs for reference
        self.config.coolify.environment_dev_id = Some(dev_env_id);
        self.config.coolify.environment_prod_id = Some(prod_env_id);
        self.config.save()?;

        cliclack::log::info(&format!(
            "Successfully created Coolify project: {} with dev and prod environments",
            project_id
        ))?;
        debug!("Coolify project config: {:?}", &project_config.coolify);
        debug!("Coolify global config: {:?}", &project_config.coolify);

        Ok(())
    }

    /// Adds GitHub app applications to Coolify environments after GitHub setup
    fn setup_coolify_github_resources(
        &self,
        project_config: &ProjectConfig,
        github_owner: &str,
        github_repo: &str,
    ) -> Result<()> {
        debug!("Setting up GitHub app applications in Coolify environments");

        let Some(project_id) = &project_config.coolify.project_id else {
            log::warn!("No Coolify project ID found, skipping GitHub app application setup");
            return Ok(());
        };

        let Some(endpoint) = project_config.coolify.api_endpoint.as_deref() else {
            log::warn!("No Coolify endpoint found, skipping GitHub app application setup");
            return Ok(());
        };

        let Ok(Some(token)) = self.config.get_coolify_token() else {
            log::warn!("No Coolify token found, skipping GitHub app application setup");
            return Ok(());
        };

        let client = CoolifyClient::new(endpoint.to_string(), token);

        // List and prompt for GitHub app
        cliclack::log::info("Loading available GitHub apps...")?;
        let github_apps = client
            .list_github_apps()
            .context("Failed to list GitHub apps")?;

        let github_app_uuid =
            prompt_github_app_selection(&github_apps).context("Failed to select GitHub app")?;

        // List and prompt for server
        cliclack::log::info("Loading available servers...")?;
        let servers = client.list_servers().context("Failed to list servers")?;

        let server = prompt_server_selection(&servers).context("Failed to select server")?;

        let server_name = server
            .name
            .clone()
            .unwrap_or_else(|| "selected server".to_string());

        let server_uuid = server
            .uuid
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Server missing UUID"))?;

        let destination_uuid = match &server.destinations {
            Some(destinations) if destinations.len() > 1 => Some(
                prompt_destination_selection(&server_name, destinations)
                    .context("Failed to select destination")?,
            ),
            Some(destinations) if destinations.len() == 1 => {
                destinations[0].uuid.clone().or_else(|| {
                    log::warn!(
                        "Destination '{}' is missing a UUID, continuing without destination selection",
                        destinations[0]
                            .name
                            .clone()
                            .unwrap_or_else(|| "unknown".to_string())
                    );
                    None
                })
            }
            Some(_) => None,
            None => {
                cliclack::log::info(&format!(
                    "Coolify API did not return destination metadata for server '{}'. \
                     If this server has multiple destinations, please provide the UUID manually.",
                    server_name
                ))?;

                let manual_destination: String = cliclack::input(
                    "Destination UUID (optional, press enter to continue without it):",
                )
                .required(false)
                .interact()
                .map_err(|e| anyhow::anyhow!("Input error: {}", e))?;

                if manual_destination.trim().is_empty() {
                    None
                } else {
                    Some(manual_destination.trim().to_string())
                }
            }
        };

        // Prompt for application name
        let app_name: String = cliclack::input("Application name:")
            .placeholder(&format!("{}-app", github_repo))
            .default_input(&format!("{}-app", github_repo))
            .validate(|input: &String| {
                if input.is_empty() {
                    Err("Application name is required")
                } else {
                    Ok(())
                }
            })
            .interact()
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?;

        // Coolify requires that the application container exposes port 80 internally.
        // Do not prompt the user; always use 80 as the exposed container port.
        let ports_exposes: String = "80".to_string();

        let repository = format!("{}/{}", github_owner, github_repo);
        // Use Coolify-specific compose file which exposes port 80 without publishing it
        let docker_compose_location = "/compose-coolify.yaml";

        // Get environment UUIDs
        let dev_env_uuid = self
            .config
            .coolify
            .environment_dev_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Dev environment UUID not found"))?;

        let prod_env_uuid = self
            .config
            .coolify
            .environment_prod_id
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Prod environment UUID not found"))?;

        // Create GitHub app application for dev environment
        cliclack::log::info("Creating GitHub app application for dev environment...")?;

        cliclack::log::warning(format!("destination_uuid: {:?}", destination_uuid.as_deref()))?;
        let _dev_app_uuid = match client.create_github_app_application(
            project_id,
            &server_uuid,
            "dev",
            dev_env_uuid,
            &github_app_uuid,
            &repository,
            "dev",
            &app_name,
            docker_compose_location,
            &ports_exposes,
            destination_uuid.as_deref(),
        ) {
            Ok(uuid) => {
                cliclack::log::success("GitHub app application created for dev environment")?;
                uuid
            }
            Err(e) => {
                log::warn!("Failed to create GitHub app application for dev: {}", e);
                cliclack::log::warning(&format!(
                    "Failed to create GitHub app application for dev: {}",
                    e
                ))?;
                return Ok(());
            }
        };

        // Create GitHub app application for prod environment
        cliclack::log::info("Creating GitHub app application for prod environment...")?;

        let _prod_app_uuid = match client.create_github_app_application(
            project_id,
            &server_uuid,
            "prod",
            prod_env_uuid,
            &github_app_uuid,
            &repository,
            "prod",
            &app_name,
            docker_compose_location,
            &ports_exposes,
            destination_uuid.as_deref(),
        ) {
            Ok(uuid) => {
                cliclack::log::success("GitHub app application created for prod environment")?;
                uuid
            }
            Err(e) => {
                log::warn!("Failed to create GitHub app application for prod: {}", e);
                cliclack::log::warning(&format!(
                    "Failed to create GitHub app application for prod: {}",
                    e
                ))?;
                return Ok(());
            }
        };

        Ok(())
    }

    fn prompt_and_validate_coolify_settings(&mut self) -> Result<(String, String, Option<String>)> {
        loop {
            debug!("Coolify default config: {:?}", self.config.coolify);
            let server_default = self.config.defaults.get("coolify_server_id").cloned();
            let (api_endpoint, api_token, server_id) = prompt_coolify_config(
                self.config.coolify.api_endpoint.as_deref(),
                self.config.coolify.token.as_deref(),
                server_default.as_deref(),
            )
            .context("Failed to prompt for Coolify settings")?;

            let client = CoolifyClient::new(api_endpoint.clone(), api_token.clone());

            match client.check_connection() {
                Ok(()) => {
                    self.persist_coolify_defaults(&api_endpoint, &api_token, server_id.clone())?;
                    return Ok((api_endpoint, api_token, server_id));
                }
                Err(err) => {
                    cliclack::log::warning(&format!(
                        "Coolify settings failed validation: {}",
                        err
                    ))?;
                    cliclack::log::info("Let's try entering the Coolify settings again.")?;
                }
            }
        }
    }

    fn persist_coolify_defaults(
        &mut self,
        endpoint: &str,
        token: &str,
        server_id: Option<String>,
    ) -> Result<()> {
        self.config.coolify.api_endpoint = Some(endpoint.to_string());

        match server_id {
            Some(value) if !value.is_empty() => {
                self.config
                    .defaults
                    .insert("coolify_server_id".into(), value);
            }
            _ => {
                self.config.defaults.remove("coolify_server_id");
            }
        }

        self.config.set_coolify_token(Some(token.to_string()))?;

        Ok(())
    }

    fn commit_project_config(&self, repo: &git2::Repository) -> Result<()> {
        debug!("Committing project configuration");

        let signature = git::get_signature(repo)?;
        let mut index = repo.index().context("Failed to get repository index")?;

        // Add .ncl/config.toml
        let config_path = self.path.join(".ncl").join("config.toml");
        if config_path.exists() {
            index
                .add_path(&std::path::Path::new(".ncl/config.toml"))
                .context("Failed to add config to index")?;
        }

        index.write().context("Failed to write index")?;

        let tree_id = index.write_tree().context("Failed to write tree")?;

        let tree = repo.find_tree(tree_id).context("Failed to find tree")?;

        let head = repo.head().context("Failed to get HEAD")?;

        let head_commit = head
            .target()
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
        git::push_to_remote(repo, "origin", "dev", true).context("Failed to push dev branch")?;

        // Push prod branch (with upstream)
        git::push_to_remote(repo, "origin", "prod", true).context("Failed to push prod branch")?;

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
