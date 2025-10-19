use std::fs;
use console::style;
use sanitize_filename::is_sanitized;
use sanitize_filename::sanitize;

use crate::modules::templates::Template;
use crate::modules::check::*;
use crate::modules::scaffold::ProjectOptions;

pub fn run() -> std::io::Result<()>{

    cliclack::clear_screen()?;
    cliclack::intro(style(" init ").on_green().black())?;

    // -------------------- Name and Path -------------------- 
    let cwd = std::env::current_dir()?;
    let project_name: String =  cliclack::input("Name your project:")
        .placeholder("awesome-project")
        .validate(move |input: &String| {
            if input.is_empty() {
                Err("Please enter a name.")
            } else if !is_sanitized(input) {
                Err("Please enter a valid name.")
            } else if fs::exists(std::env::current_dir().unwrap().join(input)).unwrap() && fs::read_dir(std::env::current_dir().unwrap().join(input)).unwrap().count() >= 1 {
                Err("Directory is non-empty.")
            } else {
                Ok(())
            }
        })
        .interact()?;


    // -------------------- Template -------------------- 
    let mut project_type = cliclack::select(format!("Pick a project type:"));

    let templates: Vec<Template> = crate::modules::templates::load_templates()?.into_iter().filter(|t| t.name != "Universal Base").collect();

    for template in templates {
        let name = template.name.clone();
        let comment = format!("{} - {}", template.comment.clone(), template.path.to_str().expect("Template should have a path"));
        project_type = project_type.item(template, name, comment);
    }
        
    let template = project_type.interact()?;

    // -------------------- Dependencies -------------------- 

    // download and install missing project depencencies
    let missing_dependencies = missing_dependencies(&template);

    if missing_dependencies.len() >= 1 {
        cliclack::note("Missing dependencies!", 
            missing_dependencies.iter().fold("".to_string(), |acc, e| format!("{acc}{} -> install: {}\n", e.name, e.install_hint))
        )?;
        // install = cliclack::confirm("Install missing dependencies?").interact()?;
    }

    
    // ------------------------------------------------------ 

    let project_options = ProjectOptions {
        name: project_name.clone(),
        path: cwd.join(project_name),
        template: template,
    };

    project_options.initialize_project()?;

    // ------------------------------------------------------ 

    let next_steps = format!(
        "cd ./{path}\nncl run dev",
        path = sanitize(&project_options.name)
    );

    cliclack::note("Next steps.", next_steps)?;
    cliclack::outro("Sucessfully initialized the project!")?;

    Ok(())
}


