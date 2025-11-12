use crate::modules::{common::Dependency, templates::Template};

/// Checks if a command is available in the system PATH
pub fn is_command_available(command: &str) -> bool {
    which::which(command).is_ok()
}

/// Returns a list of missing dependencies for a given template
pub fn missing_dependencies(template: &Template) -> Vec<Dependency> {
    template
        .dependencies
        .iter()
        .filter(|dep| !is_command_available(&dep.name))
        .cloned()
        .collect()
}

