use crate::modules::{common::Dependency, templates::Template};

pub fn is_command_available(command: &str) -> bool {
    which::which(command).is_ok()
}

pub fn missing_dependencies(template: &Template) -> Vec<Dependency> {
    let mut missing_dependencies = vec![];
    for dep in &template.dependencies {
        if !is_command_available(&dep.name) {
            missing_dependencies.push(dep.clone());
        }
    }
    missing_dependencies
}

// pub fn check_dependency(dep: &Dependency) -> bool {
//     if !is_command_available(&dep.name) { return false; }
//
//     let current_version: i32 = 0;
//     let command_output = std::process::Command::new(&dep.name)
//         .arg("--version")
//         .output();
//     match command_output {
//         Err(_) => return true,
//         Ok(output) => output.stdout.as_ascii_unchecked().as_str().chars().filter(|c| c.is_digit(10))
//     }
//
//         // .unwrap_or(false);
//
//     match &dep.target_version {
//         None => return true,
//         Some(target_version) => {
//             if target_version > current_version{
//
//             }
//         }
//     }
// }

