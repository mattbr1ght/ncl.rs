use anyhow::{Context, Result};
use log::debug;
use serde::{Deserialize, Serialize};

/// Represents a GitHub organization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitHubOrganization {
    pub login: String,
    pub id: u64,
}

/// Represents a repository creation request
#[derive(Debug, Clone, Serialize)]
pub struct CreateRepoRequest {
    pub name: String,
    pub private: bool,
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_branch: Option<String>,
}

/// Represents the response from creating a repository
#[derive(Debug, Clone, Deserialize)]
pub struct CreateRepoResponse {
    pub clone_url: String,
    pub html_url: String,
}

/// Fetches all organizations available to the authenticated user
pub fn fetch_organizations(token: &str) -> Result<Vec<GitHubOrganization>> {
    debug!("Fetching GitHub organizations");
    let client = reqwest::blocking::Client::new();

    let response = client
        .get("https://api.github.com/user/orgs")
        .header("User-Agent", "NCL-CLI")
        .header("Accept", "application/vnd.github.v3+json")
        .bearer_auth(token)
        .send()
        .context("Failed to send request to GitHub API")?;

    if !response.status().is_success() {
        let error_text = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Failed to fetch organizations: {}",
            error_text
        ));
    }

    let orgs: Vec<GitHubOrganization> = response
        .json()
        .context("Failed to parse GitHub API response")?;

    debug!("Found {} organizations", orgs.len());
    Ok(orgs)
}

/// Creates a repository in a GitHub organization or user account
pub fn create_repository(
    token: &str,
    owner: Option<&str>,
    request: &CreateRepoRequest,
) -> Result<CreateRepoResponse> {
    debug!(
        "Creating repository '{}' in {}",
        request.name,
        owner.unwrap_or("user")
    );

    let client = reqwest::blocking::Client::new();

    // Determine the API endpoint
    let url = if let Some(org) = owner {
        format!("https://api.github.com/orgs/{}/repos", org)
    } else {
        "https://api.github.com/user/repos".to_string()
    };

    let response = client
        .post(&url)
        .header("User-Agent", "NCL-CLI")
        .header("Accept", "application/vnd.github.v3+json")
        .bearer_auth(token)
        .json(request)
        .send()
        .context("Failed to send request to GitHub API")?;

    if !response.status().is_success() {
        let error_text = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Failed to create repository: {}",
            error_text
        ));
    }

    let repo_response: CreateRepoResponse = response
        .json()
        .context("Failed to parse GitHub API response")?;

    debug!(
        "Successfully created repository: {}",
        repo_response.html_url
    );
    Ok(repo_response)
}

/// Sets the default branch for a repository
pub fn set_default_branch(token: &str, owner: &str, repo: &str, branch: &str) -> Result<()> {
    debug!(
        "Setting default branch to '{}' for {}/{}",
        branch, owner, repo
    );

    let client = reqwest::blocking::Client::new();

    // First, we need to create the branch if it doesn't exist
    // Then update the default branch via PATCH /repos/{owner}/{repo}
    let url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    let response = client
        .patch(&url)
        .header("User-Agent", "NCL-CLI")
        .header("Accept", "application/vnd.github.v3+json")
        .bearer_auth(token)
        .json(&serde_json::json!({
            "default_branch": branch
        }))
        .send()
        .context("Failed to send request to GitHub API")?;

    if !response.status().is_success() {
        let error_text = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Failed to set default branch: {}",
            error_text
        ));
    }

    debug!("Successfully set default branch to '{}'", branch);
    Ok(())
}

/// Adds branch protection rules
pub fn add_branch_protection(
    token: &str,
    owner: &str,
    repo: &str,
    branch: &str,
    require_pr: bool,
    require_status_checks: bool,
    enforce_admins: bool,
) -> Result<()> {
    debug!(
        "Adding branch protection for '{}' in {}/{}",
        branch, owner, repo
    );

    let client = reqwest::blocking::Client::new();

    let url = format!(
        "https://api.github.com/repos/{}/{}/branches/{}/protection",
        owner, repo, branch
    );

    let protection_rules = serde_json::json!({
        "required_status_checks": if require_status_checks {
            serde_json::json!({
                "strict": true,
                "contexts": []
            })
        } else {
            serde_json::Value::Null
        },
        "enforce_admins": enforce_admins,
        "required_pull_request_reviews": if require_pr {
            serde_json::json!({
                "required_approving_review_count": 1,
                "dismiss_stale_reviews": true,
                "require_code_owner_reviews": false
            })
        } else {
            serde_json::Value::Null
        },
        "restrictions": null,
        "allow_force_pushes": false,
        "allow_deletions": false
    });

    let response = client
        .put(&url)
        .header("User-Agent", "NCL-CLI")
        .header("Accept", "application/vnd.github.v3+json")
        .bearer_auth(token)
        .json(&protection_rules)
        .send()
        .context("Failed to send request to GitHub API")?;

    if !response.status().is_success() {
        let error_text = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Failed to add branch protection: {}",
            error_text
        ));
    }

    debug!("Successfully added branch protection for '{}'", branch);
    Ok(())
}

/// Enables GitHub Actions for a repository.
///
/// This is a best-effort helper – many organizations enforce Actions/Workflow
/// settings at the org level, so these calls may legitimately return 403/404.
/// We treat those as non-fatal and only log them at debug level.
pub fn enable_github_actions(token: &str, owner: &str, repo: &str) -> Result<()> {
    debug!("Enabling GitHub Actions for {}/{}", owner, repo);

    let client = reqwest::blocking::Client::new();

    // 1) Best-effort: repo-level Actions permissions
    {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/permissions",
            owner, repo
        );

        let response = client
            .put(&url)
            .header("User-Agent", "NCL-CLI")
            .header("Accept", "application/vnd.github.v3+json")
            .bearer_auth(token)
            .json(&serde_json::json!({
                "enabled": true,
                "allowed_actions": "all"
            }))
            .send()
            .context("Failed to send request to GitHub API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            debug!(
                "Failed to set Actions permissions for {}/{} (status {}): {} \
                 (this is often expected if the org enforces settings)",
                owner,
                repo,
                status,
                error_text
            );
        } else {
            debug!("Successfully set Actions permissions for {}/{}", owner, repo);
        }
    }

    // 2) Best-effort: workflow permissions (needed on some orgs)
    {
        let url = format!(
            "https://api.github.com/repos/{}/{}/actions/permissions/workflow",
            owner, repo
        );

        let response = client
            .put(&url)
            .header("User-Agent", "NCL-CLI")
            .header("Accept", "application/vnd.github.v3+json")
            .bearer_auth(token)
            .json(&serde_json::json!({
                "can_approve_pull_request_reviews": true,
                "default_workflow_permissions": "write"
            }))
            .send()
            .context("Failed to send request to GitHub API")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_default();
            debug!(
                "Failed to set workflow permissions for {}/{} (status {}): {} \
                 (this is often expected if the org enforces settings)",
                owner,
                repo,
                status,
                error_text
            );
        } else {
            debug!(
                "Successfully set workflow permissions for {}/{}",
                owner, repo
            );
        }
    }

    Ok(())
}



/// Deletes a GitHub repository
pub fn delete_repository(token: &str, owner: &str, repo: &str) -> Result<()> {
    debug!("Deleting repository {}/{}", owner, repo);

    let client = reqwest::blocking::Client::new();
    let url = format!("https://api.github.com/repos/{}/{}", owner, repo);

    let response = client
        .delete(&url)
        .header("User-Agent", "NCL-CLI")
        .header("Accept", "application/vnd.github.v3+json")
        .bearer_auth(token)
        .send()
        .context("Failed to delete repository")?;

    if !response.status().is_success() {
        let error_text = response.text().unwrap_or_default();
        return Err(anyhow::anyhow!(
            "Failed to delete repository: {}",
            error_text
        ));
    }

    debug!("Successfully deleted repository");
    Ok(())
}

/// Prompts user to select an organization from the list
pub fn prompt_organization_selection(orgs: &[GitHubOrganization]) -> Result<Option<String>> {
    if orgs.is_empty() {
        cliclack::log::warning("No organizations found. Creating repository in personal account.")?;
        return Ok(None);
    }

    let mut selector = cliclack::select("Select an organization:");

    for org in orgs {
        selector = selector.item(org.login.clone(), org.login.clone(), "");
    }

    let selected = selector
        .interact()
        .map_err(|e| anyhow::anyhow!("Selection error: {}", e))?;

    Ok(Some(selected))
}
