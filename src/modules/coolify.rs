use anyhow::{Context, Result};
use log::{debug, warn};
use reqwest::StatusCode;
use serde::{Deserialize, Serialize};

/// Coolify API client for creating and managing projects
pub struct CoolifyClient {
    api_endpoint: String,
    api_token: String,
}

#[derive(Serialize)]
struct CreateProjectRequest {
    name: String,
    description: Option<String>,
}

#[derive(Deserialize)]
struct CreateProjectResponse {
    id: Option<String>,
    #[serde(rename = "uuid")]
    uuid: Option<String>,
    message: Option<String>,
}

#[derive(Serialize)]
struct CreateEnvironmentRequest {
    name: String,
}

#[derive(Deserialize, Debug)]
pub struct EnvironmentResponse {
    pub id: Option<String>,
    pub uuid: Option<String>,
    pub webhook_url: Option<String>,
    pub name: Option<String>,
}

#[derive(Serialize)]
struct CreateDeploymentRequest {
    branch: String,
    environment: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GitHubApp {
    pub uuid: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "app_id")]
    pub app_id: Option<u64>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Server {
    pub uuid: Option<String>,
    pub name: Option<String>,
    #[serde(default)]
    pub destinations: Option<Vec<Destination>>,
}

#[derive(Deserialize, Debug, Clone)]
pub struct Destination {
    pub uuid: Option<String>,
    pub name: Option<String>,
    #[serde(rename = "destination_type")]
    pub destination_type: Option<String>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ApplicationEnvironmentVariable {
    pub key: String,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_build_time: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_multiline: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_preview: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_literal: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_shown_once: Option<bool>,
}

impl ApplicationEnvironmentVariable {
    pub fn new<K: Into<String>, V: Into<String>>(key: K, value: V) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            is_build_time: None,
            is_multiline: None,
            is_preview: None,
            is_literal: None,
            is_shown_once: None,
        }
    }
}

impl ApplicationEnvironmentVariable {
    /// Convenience constructor that sets Coolify-friendly defaults
    /// is_preview: true, is_literal: true, is_multiline: true, is_shown_once: true
    pub fn with_coolify_defaults<K: Into<String>, V: Into<String>>(key: K, value: V) -> Self {
        Self {
            key: key.into(),
            value: value.into(),
            is_build_time: None,
            is_multiline: Some(true),
            is_preview: Some(true),
            is_literal: Some(true),
            is_shown_once: Some(true),
        }
    }
}

#[derive(Serialize)]
struct ApplicationEnvPayload {
    pub key: String,
    pub value: String,
    pub is_preview: bool,
    pub is_literal: bool,
    pub is_multiline: bool,
    pub is_shown_once: bool,
}

impl CoolifyClient {
    pub fn new(api_endpoint: String, api_token: String) -> Self {
        Self {
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_token,
        }
    }

    /// Performs a lightweight call to verify API connectivity and credentials
    pub fn check_connection(&self) -> Result<()> {
        let client = reqwest::blocking::Client::new();

        let response = client
            .get(&format!("{}/api/v1/projects", self.api_endpoint))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .send()
            .context("Failed to reach Coolify API")?;

        let status = response.status();

        if status.is_success() {
            return Ok(());
        }

        if status == StatusCode::UNAUTHORIZED {
            return Err(anyhow::anyhow!(
                "Coolify API rejected the token (unauthorized)"
            ));
        }

        let error_text = response.text().unwrap_or_default();
        Err(anyhow::anyhow!(format!(
            "Coolify API returned {}: {}",
            status, error_text
        )))
    }

    /// Creates a new project in Coolify
    pub fn create_project(&self, name: &str) -> Result<String> {
        debug!("Creating Coolify project: {}", name);

        let client = reqwest::blocking::Client::new();

        let request = CreateProjectRequest {
            name: name.to_string(),
            description: Some(format!("Auto-created by NCL for {}", name)),
        };

        let response = client
            .post(&format!("{}/api/v1/projects", self.api_endpoint))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to create Coolify project: {}",
                error_text
            ));
        }

        let project_response: CreateProjectResponse = response
            .json()
            .context("Failed to parse Coolify API response")?;

        let project_id = project_response
            .id
            .or(project_response.uuid)
            .ok_or_else(|| anyhow::anyhow!("Project ID not found in response"))?;

        debug!("Successfully created Coolify project: {}", project_id);
        Ok(project_id)
    }

    /// Creates an environment in a Coolify project
    pub fn create_environment(&self, project_id: &str, environment_name: &str) -> Result<String> {
        debug!(
            "Creating environment '{}' in project {}",
            environment_name, project_id
        );

        let client = reqwest::blocking::Client::new();

        let request = CreateEnvironmentRequest {
            name: environment_name.to_string(),
        };

        let response = client
            .post(&format!(
                "{}/api/v1/projects/{}/environments",
                self.api_endpoint, project_id
            ))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to create environment: {}",
                error_text
            ));
        }

        let env_response: EnvironmentResponse = response
            .json()
            .context("Failed to parse Coolify API response")?;

        let env_id = env_response
            .id
            .or(env_response.uuid)
            .ok_or_else(|| anyhow::anyhow!("Environment ID not found in response"))?;

        debug!("Successfully created environment: {}", env_id);
        Ok(env_id)
    }

    /// Gets webhook URL for an environment
    pub fn get_environment_webhook(
        &self,
        project_id: &str,
        environment_name: &str,
    ) -> Result<Option<String>> {
        debug!(
            "Getting webhook for environment '{}' in project {}",
            environment_name, project_id
        );

        let client = reqwest::blocking::Client::new();

        let response = client
            .get(&format!(
                "{}/api/v1/projects/{}/environments/{}",
                self.api_endpoint, project_id, environment_name
            ))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            debug!("Failed to get environment webhook: {}", error_text);
            return Ok(None);
        }

        let env_response: EnvironmentResponse = response
            .json()
            .context("Failed to parse Coolify API response")?;

        Ok(env_response.webhook_url)
    }

    /// Lists all environments in a project
    pub fn list_environments(&self, project_id: &str) -> Result<Vec<EnvironmentResponse>> {
        debug!("Listing environments for project {}", project_id);

        let client = reqwest::blocking::Client::new();

        let response = client
            .get(&format!(
                "{}/api/v1/projects/{}/environments",
                self.api_endpoint, project_id
            ))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to list environments: {}",
                error_text
            ));
        }

        let environments: Vec<EnvironmentResponse> = response
            .json()
            .context("Failed to parse Coolify API response")?;

        debug!("Found {} environments", environments.len());
        Ok(environments)
    }

    /// Deletes an environment from a project
    pub fn delete_environment(&self, project_id: &str, environment_id: &str) -> Result<()> {
        debug!(
            "Deleting environment {} from project {}",
            environment_id, project_id
        );

        let client = reqwest::blocking::Client::new();

        let response = client
            .delete(&format!(
                "{}/api/v1/projects/{}/environments/{}",
                self.api_endpoint, project_id, environment_id
            ))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to delete environment: {}",
                error_text
            ));
        }

        debug!("Successfully deleted environment");
        Ok(())
    }

    /// Lists all available GitHub apps
    pub fn list_github_apps(&self) -> Result<Vec<GitHubApp>> {
        debug!("Listing GitHub apps");

        let client = reqwest::blocking::Client::new();

        let response = client
            .get(&format!("{}/api/v1/github-apps", self.api_endpoint))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to list GitHub apps: {}",
                error_text
            ));
        }

        let apps: Vec<GitHubApp> = response
            .json()
            .context("Failed to parse Coolify API response")?;

        debug!("Found {} GitHub apps", apps.len());
        Ok(apps)
    }

    /// Lists all available servers
    pub fn list_servers(&self) -> Result<Vec<Server>> {
        debug!("Listing servers");

        let client = reqwest::blocking::Client::new();

        let response = client
            .get(&format!("{}/api/v1/servers", self.api_endpoint))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to list servers: {}", error_text));
        }

        let servers: Vec<Server> = response
            .json()
            .context("Failed to parse Coolify API response")?;

        debug!("Found {} servers", servers.len());
        Ok(servers)
    }

    /// Creates a GitHub app application in an environment
    pub fn create_github_app_application(
        &self,
        project_uuid: &str,
        server_uuid: &str,
        environment_name: &str,
        environment_uuid: &str,
        github_app_uuid: &str,
        git_repository: &str,
        git_branch: &str,
        name: &str,
        docker_compose_location: &str,
        ports_exposes: &str,
        destination_uuid: Option<&str>,
        environment_variables: &[ApplicationEnvironmentVariable],
    ) -> Result<String> {
        debug!(
            "Creating GitHub app application '{}' in environment {} for project {}",
            name, environment_name, project_uuid
        );

        #[derive(Serialize)]
        struct CreateApplicationRequest {
            project_uuid: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            server_uuid: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            destination_uuid: Option<String>,
            environment_name: String,
            environment_uuid: String,
            github_app_uuid: String,
            git_repository: String,
            git_branch: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            git_commit_sha: Option<String>,
            build_pack: String,
            name: String,
            ports_exposes: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            docker_compose_location: Option<String>,
        }

        let client = reqwest::blocking::Client::new();


        let request = CreateApplicationRequest {
            project_uuid: project_uuid.to_string(),
            server_uuid: Some(server_uuid.to_string()),
            destination_uuid: destination_uuid.map(|value| value.to_string()),
            environment_name: environment_name.to_string(),
            environment_uuid: environment_uuid.to_string(),
            github_app_uuid: github_app_uuid.to_string(),
            git_repository: git_repository.to_string(),
            git_branch: git_branch.to_string(),
            git_commit_sha: Some("HEAD".to_string()),
            build_pack: "dockercompose".to_string(),
            name: name.to_string(),
            ports_exposes: ports_exposes.to_string(),
            docker_compose_location: Some(docker_compose_location.to_string()),
        };

        let response = client
            .post(&format!(
                "{}/api/v1/applications/private-github-app",
                self.api_endpoint
            ))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to create GitHub app application: {}",
                error_text
            ));
        }

        #[derive(Deserialize)]
        struct ApplicationResponse {
            uuid: Option<String>,
        }

        let app_response: ApplicationResponse = response
            .json()
            .context("Failed to parse Coolify API response")?;

        let app_uuid = app_response
            .uuid
            .ok_or_else(|| anyhow::anyhow!("Application UUID not found in response"))?;

        debug!("Successfully created GitHub app application: {}", app_uuid);
        Ok(app_uuid)
    }

    /// Sets environment variables for an application from Coolify's perspective.
    /// Expects values already normalized to the Coolify payload schema.
    pub fn set_application_envs(
        &self,
        application_uuid: &str,
        envs: &[ApplicationEnvironmentVariable],
    ) -> Result<()> {
        debug!(
            "Setting {} environment variables for application {}",
            envs.len(),
            application_uuid
        );

        let client = reqwest::blocking::Client::new();

        let payload: Vec<ApplicationEnvPayload> = envs
            .iter()
            .map(|e| ApplicationEnvPayload {
                key: e.key.clone(),
                value: e.value.clone(),
                is_preview: e.is_preview.unwrap_or(true),
                is_literal: e.is_literal.unwrap_or(true),
                is_multiline: e.is_multiline.unwrap_or(true),
                is_shown_once: e.is_shown_once.unwrap_or(true),
            })
            .collect();

        let response = client
            .post(&format!(
                "{}/api/v1/applications/{}/envs",
                self.api_endpoint, application_uuid
            ))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .context("Failed to send request to Coolify API for setting envs")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!(
                "Failed to set application environment variables: {}",
                error_text
            ));
        }

        debug!("Successfully updated application environment variables");
        Ok(())
    }

    /// Deletes a Coolify project
    pub fn delete_project(&self, project_id: &str) -> Result<()> {
        debug!("Deleting Coolify project: {}", project_id);

        let client = reqwest::blocking::Client::new();

        let response = client
            .delete(&format!(
                "{}/api/v1/projects/{}",
                self.api_endpoint, project_id
            ))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .send()
            .context("Failed to send request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to delete project: {}", error_text));
        }

        debug!("Successfully deleted Coolify project");
        Ok(())
    }
}

/// Prompts user if they want to set up Coolify
pub fn prompt_coolify_setup() -> Result<bool> {
    cliclack::confirm("Would you like to set up Coolify integration?")
        .initial_value(false)
        .interact()
        .map_err(|e| anyhow::anyhow!("Input error: {}", e))
}

/// Prompts user for Coolify configuration
pub fn prompt_coolify_config(
    default_endpoint: Option<&str>,
    default_token: Option<&str>,
    default_server_id: Option<&str>,
) -> Result<(String, String, Option<String>)> {
    let endpoint = if let Some(default) = default_endpoint {
        cliclack::input("Coolify API URL - (you can press enter for the default value below):")
            .placeholder(default)
            .default_input(default)
            .validate(|input: &String| {
                if !input.starts_with("http://") && !input.starts_with("https://") {
                    Err("URL must start with http:// or https://")
                } else {
                    Ok(())
                }
            })
            .interact()
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?
    } else {
        cliclack::input("Coolify API URL:")
            .placeholder("https://coolify.example.com")
            .validate(|input: &String| {
                if input.is_empty() {
                    Err("API URL is required")
                } else if !input.starts_with("http://") && !input.starts_with("https://") {
                    Err("URL must start with http:// or https://")
                } else {
                    Ok(())
                }
            })
            .interact()
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?
    };

    let token = if let Some(default) = default_token {
        cliclack::input("Coolify API Token - (you can press enter for the default value below):")
            .placeholder(default)
            .default_input(default)
            .validate(|input: &String| {
                if input.is_empty() {
                    Err("API token is required")
                } else {
                    Ok(())
                }
            })
            .interact()
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?
    } else {
        cliclack::input("Coolify API Token:")
            .placeholder("your-api-token")
            .validate(|input: &String| {
                if input.is_empty() {
                    Err("API token is required")
                } else {
                    Ok(())
                }
            })
            .interact()
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?
    };

    let server_id: String = if let Some(default) = default_server_id {
        cliclack::input(
            "Coolify Server ID (optional) - (you can press enter for the default value below):",
        )
        .placeholder(default)
        .default_input(default)
        .required(false)
        .interact()
        .map_err(|e| anyhow::anyhow!("Input error: {}", e))?
    } else {
        cliclack::input("Coolify Server ID (optional):")
            .placeholder("Leave empty if not needed")
            .required(false)
            .interact()
            .map_err(|e| anyhow::anyhow!("Input error: {}", e))?
    };

    let server_id_opt = if server_id.is_empty() {
        None
    } else {
        Some(server_id)
    };

    Ok((endpoint, token, server_id_opt))
}

/// Prompts user to select a GitHub app from a list
pub fn prompt_github_app_selection(apps: &[GitHubApp]) -> Result<String> {
    if apps.is_empty() {
        return Err(anyhow::anyhow!(
            "No GitHub apps found. Please create a GitHub app in Coolify first."
        ));
    }

    let mut selector = cliclack::select("Select a GitHub App:");

    for app in apps {
        let display_name = app
            .name
            .clone()
            .or_else(|| app.app_id.map(|id| format!("App ID: {}", id)))
            .unwrap_or_else(|| "Unknown".to_string());

        let uuid = app
            .uuid
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("GitHub app missing UUID"))?;

        selector = selector.item(uuid.clone(), display_name, "");
    }

    let selected_uuid = selector
        .interact()
        .map_err(|e| anyhow::anyhow!("Selection error: {}", e))?;

    Ok(selected_uuid)
}

/// Prompts user to select a server from a list
pub fn prompt_server_selection(servers: &[Server]) -> Result<Server> {
    if servers.is_empty() {
        return Err(anyhow::anyhow!(
            "No servers found. Please add a server in Coolify first."
        ));
    }

    let mut selector = cliclack::select("Select a server:");

    for server in servers {
        let display_name = server
            .name
            .clone()
            .unwrap_or_else(|| "Unknown Server".to_string());

        let uuid = server
            .uuid
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Server missing UUID"))?;

        selector = selector.item(uuid.clone(), display_name, "");
    }

    let selected_uuid = selector
        .interact()
        .map_err(|e| anyhow::anyhow!("Selection error: {}", e))?;

    let server = servers
        .iter()
        .find(|server| server.uuid.as_deref() == Some(&selected_uuid))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("Unable to load selected server"))?;

    Ok(server)
}

pub fn prompt_destination_selection(
    server_name: &str,
    destinations: &[Destination],
) -> Result<String> {
    if destinations.is_empty() {
        return Err(anyhow::anyhow!(
            "No destinations available for server {}",
            server_name
        ));
    }

    let mut selector = cliclack::select(&format!(
        "Select a destination for server '{}':",
        server_name
    ));

    for destination in destinations {
        let display_name = destination
            .name
            .clone()
            .or_else(|| destination.destination_type.clone())
            .unwrap_or_else(|| "Unknown destination".to_string());

        let uuid = destination
            .uuid
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Destination missing UUID"))?;

        selector = selector.item(uuid.clone(), display_name, "");
    }

    let selected_uuid = selector
        .interact()
        .map_err(|e| anyhow::anyhow!("Selection error: {}", e))?;

    Ok(selected_uuid)
}
