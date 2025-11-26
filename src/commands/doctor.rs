use crate::modules::config::NclConfig;
use anyhow::{Context, Result};

pub fn run() -> Result<()> {
    cliclack::intro("Doctor - Validating NCL Configuration")?;

    let config = NclConfig::load().context("Failed to load NCL configuration")?;

    let mut issues = Vec::new();
    let mut warnings = Vec::new();

    // Check GitHub token
    cliclack::log::info("Checking GitHub token...")?;
    match config.get_github_token() {
        Ok(Some(token)) => {
            // Test token by fetching user info
            let client = reqwest::blocking::Client::new();
            let response = client
                .get("https://api.github.com/user")
                .header("User-Agent", "NCL-CLI")
                .header("Accept", "application/vnd.github.v3+json")
                .bearer_auth(&token)
                .send();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    cliclack::log::success("GitHub token is valid")?;
                }
                Ok(resp) => {
                    issues.push("GitHub token is invalid or expired".to_string());
                    cliclack::log::error(&format!(
                        "GitHub token validation failed: {}",
                        resp.status()
                    ))?;
                }
                Err(e) => {
                    warnings.push(format!("Could not validate GitHub token: {}", e));
                    cliclack::log::warning(&format!("Could not validate GitHub token: {}", e))?;
                }
            }
        }
        Ok(None) => {
            warnings.push("GitHub token not configured".to_string());
            cliclack::log::warning("GitHub token not configured")?;
        }
        Err(e) => {
            issues.push(format!("Failed to get GitHub token: {}", e));
            cliclack::log::error(&format!("Failed to get GitHub token: {}", e))?;
        }
    }

    // Check Coolify configuration
    cliclack::log::info("Checking Coolify configuration...")?;
    match (
        config.coolify.api_endpoint.as_ref(),
        config.get_coolify_token().ok().flatten(),
    ) {
        (Some(endpoint), Some(token)) => {
            // Try to make a simple API call to validate
            let test_client = reqwest::blocking::Client::new();
            let response = test_client
                .get(&format!("{}/api/v1/projects", endpoint))
                .header("Authorization", &format!("Bearer {}", token))
                .send();

            match response {
                Ok(resp) if resp.status().is_success() => {
                    cliclack::log::success("Coolify API is accessible")?;
                }
                Ok(resp) => {
                    warnings.push(format!("Coolify API returned error: {}", resp.status()));
                    cliclack::log::warning(&format!(
                        "Coolify API returned error: {}",
                        resp.status()
                    ))?;
                }
                Err(e) => {
                    warnings.push(format!("Could not connect to Coolify: {}", e));
                    cliclack::log::warning(&format!("Could not connect to Coolify: {}", e))?;
                }
            }
        }
        _ => {
            warnings.push("Coolify not configured".to_string());
            cliclack::log::warning("Coolify not configured (this is optional)")?;
        }
    }

    // Summary
    if issues.is_empty() && warnings.is_empty() {
        cliclack::outro("All checks passed!")?;
    } else if issues.is_empty() {
        cliclack::outro_note("Some warnings", &warnings.join("\n"))?;
    } else {
        cliclack::outro_note(
            "Issues found",
            &format!(
                "{}\n\nWarnings:\n{}",
                issues.join("\n"),
                if warnings.is_empty() {
                    "None"
                } else {
                    &warnings.join("\n")
                }
            ),
        )?;
    }

    Ok(())
}
