use anyhow::{Context, Result};
use log::debug;
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
    uuid: Option<String>,
    #[serde(rename = "uuid")]
    project_uuid: Option<String>,
    message: Option<String>,
}

#[derive(Serialize)]
struct CreateDeploymentRequest {
    branch: String,
    environment: String,
}

impl CoolifyClient {
    pub fn new(api_endpoint: String, api_token: String) -> Self {
        Self {
            api_endpoint: api_endpoint.trim_end_matches('/').to_string(),
            api_token,
        }
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
            return Err(anyhow::anyhow!("Failed to create Coolify project: {}", error_text));
        }

        let project_response: CreateProjectResponse = response.json()
            .context("Failed to parse Coolify API response")?;

        let project_id = project_response.id
            .or(project_response.uuid)
            .or(project_response.project_uuid)
            .ok_or_else(|| anyhow::anyhow!("Project ID not found in response"))?;

        debug!("Successfully created Coolify project: {}", project_id);
        Ok(project_id)
    }

    /// Triggers a deployment for a project
    pub fn trigger_deployment(&self, project_id: &str, branch: &str, environment: &str) -> Result<()> {
        debug!("Triggering deployment for project {} on branch {} (env: {})", project_id, branch, environment);

        let client = reqwest::blocking::Client::new();
        
        let deploy_request = CreateDeploymentRequest {
            branch: branch.to_string(),
            environment: environment.to_string(),
        };

        let response = client
            .post(&format!("{}/api/v1/projects/{}/deploy", self.api_endpoint, project_id))
            .header("Authorization", &format!("Bearer {}", self.api_token))
            .header("Content-Type", "application/json")
            .json(&deploy_request)
            .send()
            .context("Failed to send deployment request to Coolify API")?;

        if !response.status().is_success() {
            let error_text = response.text().unwrap_or_default();
            return Err(anyhow::anyhow!("Failed to trigger deployment: {}", error_text));
        }

        debug!("Successfully triggered deployment");
        Ok(())
    }
}

/// Prompts user for Coolify configuration
pub fn prompt_coolify_config(
    default_endpoint: Option<&str>,
    default_token: Option<&str>,
    default_server_id: Option<&str>,
) -> Result<(String, String, Option<String>)> {
    let endpoint = if let Some(default) = default_endpoint {
        cliclack::input("Coolify API URL:")
            .placeholder("https://coolify.example.com")
            .default_input(default)
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
        cliclack::input("Coolify API Token:")
            .placeholder("your-api-token")
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
        cliclack::input("Coolify Server ID (optional):")
            .placeholder("Leave empty if not needed")
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

