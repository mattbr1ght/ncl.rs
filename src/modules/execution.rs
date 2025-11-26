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
    debug!(
        "Executing command: {:?} in {}",
        spec,
        project_path.display()
    );

    match &spec.context {
        ExecutionContext::Host => {
            // Check if this is a docker compose command that needs project name
            let cmd = &spec.command;
            if (cmd.contains("docker compose") || cmd.contains("docker-compose"))
                && (cmd.contains("up")
                    || cmd.contains("down")
                    || cmd.contains("start")
                    || cmd.contains("stop"))
            {
                execute_docker_compose_command(cmd, project_path)
            } else {
                execute_host_command(cmd, project_path)
            }
        }
        ExecutionContext::Docker { container, service } => {
            execute_docker_command(&spec.command, project_path, container, service.as_deref())
        }
    }
}

/// Execute docker compose commands with proper project name
fn execute_docker_compose_command(cmd: &str, project_path: &Path) -> Result<Output> {
    check_docker_permissions()?;
    let project_name = get_project_name(project_path);
    let compose_cmd = get_compose_command()?;

    // Parse the command and rebuild it with project name
    // cmd is like "docker compose up -d" or "./vendor/bin/sail up -d"
    let parts: Vec<&str> = cmd.split_whitespace().collect();

    if cmd.contains("sail") {
        // For sail commands, just run as-is (sail handles its own project naming)
        return execute_host_command(cmd, project_path);
    }

    let mut command = Command::new(compose_cmd);
    if compose_cmd == "docker" {
        command.arg("compose");
    }

    // Add project name and compose file
    if let Some(compose_file) = find_compose_file(project_path) {
        command.arg("-p").arg(&project_name);
        command.arg("-f").arg(compose_file);
    }

    // Add the rest of the command (up, down, start, stop, etc.)
    if compose_cmd == "docker" {
        // Skip "docker compose" from the command
        if parts.len() >= 2 && parts[0] == "docker" && parts[1] == "compose" {
            for arg in parts.iter().skip(2) {
                command.arg(arg);
            }
        } else {
            // Fallback: just execute as host command
            return execute_host_command(cmd, project_path);
        }
    } else {
        // docker-compose: skip "docker-compose"
        if parts.len() >= 1 && parts[0] == "docker-compose" {
            for arg in parts.iter().skip(1) {
                command.arg(arg);
            }
        } else {
            return execute_host_command(cmd, project_path);
        }
    }

    command
        .current_dir(project_path)
        .env("COMPOSE_PROJECT_NAME", &project_name)
        .output()
        .with_context(|| format!("Failed to execute docker compose command: {}", cmd))
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

fn find_compose_file(project_path: &Path) -> Option<std::path::PathBuf> {
    let compose_file = project_path.join("compose.yaml");
    if compose_file.exists() {
        Some(compose_file)
    } else {
        let alt = project_path.join("docker-compose.yml");
        if alt.exists() { Some(alt) } else { None }
    }
}

fn get_compose_command() -> Result<&'static str> {
    if which::which("docker").is_ok() {
        Ok("docker")
    } else if which::which("docker-compose").is_ok() {
        Ok("docker-compose")
    } else {
        Err(anyhow::anyhow!(
            "Neither 'docker' nor 'docker-compose' found"
        ))
    }
}

/// Check if Docker Desktop is being used (works without sudo on all platforms)
fn is_docker_desktop() -> bool {
    // Docker Desktop sets DOCKER_HOST or uses a different context
    std::env::var("DOCKER_HOST").is_ok()
        || Command::new("docker")
            .arg("context")
            .arg("show")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.contains("desktop") || s.contains("Docker Desktop"))
            .unwrap_or(false)
}

/// Check if user has docker permissions (can access docker socket)
/// Cross-platform: Works on macOS/Windows with Docker Desktop, Linux needs docker group
fn check_docker_permissions() -> Result<()> {
    let output = Command::new("docker").arg("ps").output();

    match output {
        Ok(output) if output.status.success() => Ok(()),
        Ok(_) | Err(_) => {
            // Check if we're on Linux with traditional Docker (not Docker Desktop)
            #[cfg(target_os = "linux")]
            {
                if !is_docker_desktop() {
                    return Err(anyhow::anyhow!(
                        "Docker permission denied on Linux.\n\n\
                        Options (no additional dependencies needed):\n\n\
                        1. Use Docker Desktop for Linux (works without sudo):\n\
                           Download: https://www.docker.com/products/docker-desktop/\n\
                           Docker Desktop works without sudo on all platforms.\n\n\
                        2. Add user to docker group (one-time, requires sudo):\n\
                           sudo usermod -aG docker $USER\n\
                           Then: newgrp docker (or log out/in)\n\n\
                        3. On macOS/Windows: Docker Desktop should work automatically.\n\
                           If not, ensure Docker Desktop is running."
                    ));
                }
            }

            // macOS/Windows or Docker Desktop - provide generic error
            #[cfg(not(target_os = "linux"))]
            {
                return Err(anyhow::anyhow!(
                    "Docker is not accessible. Please ensure:\n\
                    1. Docker Desktop is installed and running\n\
                    2. Docker Desktop is started (check system tray/menu bar)\n\
                    3. Try: docker ps (to verify Docker is working)"
                ));
            }

            // Linux with Docker Desktop - should work but didn't
            #[cfg(target_os = "linux")]
            {
                Err(anyhow::anyhow!(
                    "Docker permission denied. If using Docker Desktop, ensure it's running.\n\
                    Otherwise, add user to docker group:\n\
                    sudo usermod -aG docker $USER && newgrp docker"
                ))
            }
        }
    }
}

fn ensure_service_running(
    compose_path: &Path,
    service_name: &str,
    project_path: &Path,
) -> Result<()> {
    let compose_cmd = get_compose_command()?;
    let project_name = get_project_name(project_path);

    // Check if service is running
    let mut check_cmd = Command::new(compose_cmd);
    if compose_cmd == "docker" {
        check_cmd.arg("compose");
    }
    check_cmd
        .arg("-p")
        .arg(&project_name)
        .arg("-f")
        .arg(compose_path)
        .arg("ps")
        .arg("-q")
        .arg(service_name)
        .current_dir(project_path)
        .env("COMPOSE_PROJECT_NAME", &project_name);

    let check_output = check_cmd.output().with_context(
        || "Failed to check if service is running. Make sure you have docker permissions.",
    )?;

    // If service is not running, start it
    if check_output.stdout.is_empty() {
        debug!("Service '{}' is not running, starting it...", service_name);
        let mut start_cmd = Command::new(compose_cmd);
        if compose_cmd == "docker" {
            start_cmd.arg("compose");
        }
        start_cmd
            .arg("-p")
            .arg(&project_name)
            .arg("-f")
            .arg(compose_path)
            .arg("up")
            .arg("-d")
            .arg(service_name)
            .current_dir(project_path)
            .env("COMPOSE_PROJECT_NAME", &project_name)
            .output()
            .with_context(|| {
                format!(
                    "Failed to start service '{}'. Make sure you have docker permissions.",
                    service_name
                )
            })?;

        // Wait a bit for the service to be ready
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    Ok(())
}

/// Get current user ID for Docker user mapping (prevents root-owned files)
#[cfg(unix)]
fn get_docker_user_flag() -> Option<String> {
    if let Ok(uid_output) = Command::new("id").arg("-u").output() {
        if let Ok(uid) = String::from_utf8(uid_output.stdout) {
            if let Ok(gid_output) = Command::new("id").arg("-g").output() {
                if let Ok(gid) = String::from_utf8(gid_output.stdout) {
                    return Some(format!("{}:{}", uid.trim(), gid.trim()));
                }
            }
        }
    }
    None
}

/// Get the project name from the project directory (for COMPOSE_PROJECT_NAME)
fn get_project_name(project_path: &Path) -> String {
    // Use directory name as project name, sanitized for Docker Compose
    project_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("ncl-project")
        .to_lowercase()
        .replace([' ', '.', '_'], "-")
}

fn execute_docker_command(
    cmd: &str,
    project_path: &Path,
    container: &str,
    service: Option<&str>,
) -> Result<Output> {
    // Check docker permissions first, before any docker operations
    check_docker_permissions()?;

    let compose_file = find_compose_file(project_path);
    let project_name = get_project_name(project_path);

    // Use docker-compose exec if service is specified and compose file exists
    if let (Some(service_name), Some(compose_path)) = (service, compose_file) {
        // Ensure the service is running before executing commands
        ensure_service_running(&compose_path, service_name, project_path)?;

        let compose_cmd = get_compose_command()?;
        let mut command = Command::new(compose_cmd);
        if compose_cmd == "docker" {
            command.arg("compose");
        }

        command
            .arg("-p") // Set project name to avoid conflicts
            .arg(&project_name)
            .arg("-f")
            .arg(compose_path)
            .arg("exec");

        // Try to run as current user to avoid root-owned files
        #[cfg(unix)]
        {
            if let Some(user_flag) = get_docker_user_flag() {
                command.arg("--user").arg(&user_flag);
            }
        }

        command
            .arg("-T") // Disable TTY allocation
            .arg(service_name)
            .arg("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(project_path)
            .env("COMPOSE_PROJECT_NAME", &project_name) // Also set env var for compatibility
            .output()
            .with_context(|| {
                format!(
                    "Failed to execute command in docker-compose service '{}': {}\nNote: Make sure the service is running and the command is valid.",
                    service_name, cmd
                )
            })
    } else {
        // docker exec container_name sh -c "command"
        let mut command = Command::new("docker");
        command.arg("exec");

        // Try to run as current user to avoid root-owned files
        #[cfg(unix)]
        {
            if let Some(user_flag) = get_docker_user_flag() {
                command.arg("--user").arg(&user_flag);
            }
        }

        command
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
/// Supports syntax:
/// - "command" - runs on host
/// - "docker:service:command" - runs in docker-compose service
/// - Commands containing "sail" or docker-compose management are forced to host
pub fn parse_command(cmd: &str) -> CommandSpec {
    // Commands that manage docker-compose (like sail) should run on host
    // Also handle docker compose up/down commands
    let is_docker_management = cmd.contains("sail")
        || cmd.contains("docker-compose")
        || cmd.contains("docker compose")
        || (cmd.contains("docker")
            && (cmd.contains("up")
                || cmd.contains("down")
                || cmd.contains("start")
                || cmd.contains("stop")));

    if is_docker_management {
        return CommandSpec {
            command: cmd.to_string(),
            context: ExecutionContext::Host,
            working_dir: None,
        };
    }

    if let Some(stripped) = cmd.strip_prefix("docker:") {
        if let Some((target, command)) = stripped.split_once(':') {
            CommandSpec {
                command: command.to_string(),
                context: ExecutionContext::Docker {
                    container: target.to_string(),
                    service: Some(target.to_string()),
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
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let stdout = String::from_utf8_lossy(&output.stdout);
                let error_details = if !stderr.is_empty() {
                    format!(": {}", stderr.trim())
                } else if !stdout.is_empty() {
                    format!(": {}", stdout.trim())
                } else {
                    String::new()
                };
                let error_msg = format!("Command failed: {}{}", cmd_str, error_details);
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
