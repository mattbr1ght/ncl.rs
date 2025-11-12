use anyhow::{Context, Result};
use log::debug;
use std::path::Path;
use std::process::{Command, Output};

/// Execution context for running commands
#[derive(Debug, Clone)]
pub enum ExecutionContext {
    /// Run command directly on host
    Host,
    /// Run command inside a Docker container
    Docker {
        container: String,
        service: Option<String>, // docker-compose service name
    },
}

/// A single command to execute
#[derive(Debug, Clone)]
pub struct CommandSpec {
    /// The command to run
    pub command: String,
    /// Where to execute the command
    pub context: ExecutionContext,
    /// Working directory (relative to project root)
    pub working_dir: Option<String>,
}

/// Execute a single command
pub fn execute_command(spec: &CommandSpec, project_path: &Path) -> Result<Output> {
    debug!("Executing command: {:?} in {}", spec, project_path.display());

    match &spec.context {
        ExecutionContext::Host => execute_host_command(&spec.command, project_path),
        ExecutionContext::Docker { container, service } => {
            execute_docker_command(&spec.command, project_path, container, service.as_deref())
        }
    }
}

fn execute_host_command(cmd: &str, working_directory: &Path) -> Result<Output> {
    #[cfg(windows)]
    const SHELL: &str = "cmd";
    #[cfg(not(windows))]
    const SHELL: &str = "sh";

    #[cfg(windows)]
    const CMD_SWITCH: &str = "/c";
    #[cfg(not(windows))]
    const CMD_SWITCH: &str = "-c";

    Command::new(SHELL)
        .arg(CMD_SWITCH)
        .arg(cmd)
        .current_dir(working_directory)
        .output()
        .with_context(|| format!("Failed to execute command: {}", cmd))
}

fn execute_docker_command(
    cmd: &str,
    project_path: &Path,
    container: &str,
    service: Option<&str>,
) -> Result<Output> {
    // Check if docker-compose file exists
    let compose_file = project_path.join("compose.yaml");
    let compose_file = if compose_file.exists() {
        Some(compose_file)
    } else {
        let alt = project_path.join("docker-compose.yml");
        if alt.exists() {
            Some(alt)
        } else {
            None
        }
    };

    // Use docker-compose exec if service is specified and compose file exists
    if let (Some(service_name), Some(compose_path)) = (service, compose_file) {
        // Try docker compose (newer) first, fallback to docker-compose
        let compose_cmd = if which::which("docker").is_ok() {
            "docker"
        } else if which::which("docker-compose").is_ok() {
            "docker-compose"
        } else {
            return Err(anyhow::anyhow!("Neither 'docker' nor 'docker-compose' found"));
        };

        let mut command = Command::new(compose_cmd);
        if compose_cmd == "docker" {
            command.arg("compose");
        }
        
        command
            .arg("-f")
            .arg(compose_path)
            .arg("exec")
            .arg("-T") // Disable TTY allocation
            .arg(service_name)
            .arg("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(project_path)
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute command in docker-compose service '{}': {}",
                    service_name, cmd
                )
            })
    } else {
        // docker exec container_name sh -c "command"
        Command::new("docker")
            .arg("exec")
            .arg(container)
            .arg("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(project_path)
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute command in docker container '{}': {}",
                    container, cmd
                )
            })
    }
}

/// Parse a command string into a CommandSpec
/// Supports syntax: "command" or "docker:service:command" or "docker:container:command"
pub fn parse_command(cmd: &str) -> CommandSpec {
    if let Some(stripped) = cmd.strip_prefix("docker:") {
        if let Some((target, command)) = stripped.split_once(':') {
            // Check if target looks like a service name (lowercase, hyphens) or container name
            // For now, we'll treat it as a service name if docker-compose is available
            CommandSpec {
                command: command.to_string(),
                context: ExecutionContext::Docker {
                    container: target.to_string(),
                    service: Some(target.to_string()), // Assume it's a service name
                },
                working_dir: None,
            }
        } else {
            // docker:command - use default container
            CommandSpec {
                command: stripped.to_string(),
                context: ExecutionContext::Docker {
                    container: "app".to_string(),
                    service: Some("app".to_string()),
                },
                working_dir: None,
            }
        }
    } else {
        CommandSpec {
            command: cmd.to_string(),
            context: ExecutionContext::Host,
            working_dir: None,
        }
    }
}

/// Execute multiple commands with progress indication
pub fn execute_commands(
    commands: &[String],
    project_path: &Path,
    progress_label: &str,
) -> Result<()> {
    if commands.is_empty() {
        return Ok(());
    }

    let previous_dir = std::env::current_dir()?;
    std::env::set_current_dir(project_path)?;

    let result = execute_commands_internal(commands, project_path, progress_label);

    let _ = std::env::set_current_dir(&previous_dir);
    result
}

fn execute_commands_internal(
    commands: &[String],
    project_path: &Path,
    progress_label: &str,
) -> Result<()> {
    let multi = cliclack::multi_progress(progress_label);
    let mut errors = Vec::new();

    for cmd_str in commands {
        let spec = parse_command(cmd_str);
        let spinner = multi.add(cliclack::spinner());
        spinner.start(format!("◇ {}", cmd_str));

        match execute_command(&spec, project_path) {
            Ok(output) if output.status.success() => {
                spinner.stop(format!("  ◆ {}", cmd_str));
            }
            Ok(_) => {
                let error_msg = format!("Command failed: {}", cmd_str);
                spinner.stop(format!("  △ {}", error_msg));
                errors.push(error_msg);
            }
            Err(e) => {
                let error_msg = format!("Error: {} ({})", e, cmd_str);
                spinner.stop(format!("  △ {}", error_msg));
                errors.push(error_msg);
            }
        }
    }

    multi.stop();

    if !errors.is_empty() {
        return Err(anyhow::anyhow!(
            "Some commands failed:\n{}",
            errors.join("\n")
        ));
    }

    Ok(())
}

