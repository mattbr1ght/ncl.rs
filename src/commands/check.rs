// use std::io::{Error, ErrorKind};

use crate::modules::{check::{missing_dependencies}, templates::Template};

pub fn run() -> std::io::Result<()> {
    let cwd = std::env::current_dir()?;
    if !cwd.join("template.toml").exists() {
        cliclack::outro("You are not in a valid project directory")?;
        return Ok(());
        // return Err(
        //     Error::new(
        //         ErrorKind::Other, 
        //         "You are not in a valid project directory"
        //     )
        // );
    }

    let meta = std::fs::read_to_string(cwd.join("template.toml"))?;
    let template: Template = toml::from_str(&meta).expect("Toml deserialization error");

    let missing_dependencies = missing_dependencies(&template);

    if missing_dependencies.len() >= 1 {
        cliclack::outro_note("Missing dependencies!", 
            missing_dependencies.iter().fold("".to_string(), |acc, e| format!("{acc}{} -> install: {}\n", e.name, e.install_hint))
        )?;
    } else {
        cliclack::outro("No missing dependencies!")?;
    }

    Ok(())
}
