use std::fs;
use console::style;
use sanitize_filename::is_sanitized;
use sanitize_filename::sanitize;

use crate::modules::common::{Template, Addition, Additional};

pub fn run() -> std::io::Result<()>{

    cliclack::clear_screen()?;
    cliclack::intro(style(" init ").on_green().black())?;

    // -------------------- Name and Path -------------------- 
    let cwd = std::env::current_dir()?;
    let project_name: String =  cliclack::input("Name your project:")
        .placeholder("sparkling-solid")
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

    //TODO: make a list of templates
    let mut templates: Vec<Template> = vec![];
    templates.push(Template::new("TypeScript", "ts", "", vec!["ts".to_string()]));
    templates.push(Template::new("JavaScript", "js", "", vec!["node".to_string()]));

    for template in templates {
        let name = template.name.clone();
        let comment = template.comment.clone();
        project_type = project_type.item(template, name, comment);
    }
        
    let template = project_type.interact()?;

    // -------------------- Additions -------------------- 
    let mut additions_select = cliclack::multiselect("Select additional tools:").initial_values(vec![Addition::GitHubRepo]);

    //TODO: make a list of additionals
    let mut additions: Vec<Additional> = vec![];
    additions.push(Additional::new("Prettier", Addition::Prettier, ""));
    additions.push(Additional::new("GitHub Repository", Addition::GitHubRepo, ""));
        
    for addition in additions {
        let name = addition.name.clone();
        let comment = addition.comment.clone();
        additions_select = additions_select.item(addition.addition, name, comment);
    }

    let selected_additions = additions_select.interact()?;

    // -------------------- Dependencies -------------------- 
    // TODO: fix it
    // let install;
    // if !(super::check::dependencies_installed(template, selected_additions)) {
    //     install = cliclack::confirm("Install dependencies?").interact()?;
    // } else {
    //     install = false;
    // }

    // download and install missing project depencencies
    let missing_dependencies = &template.dependencies;

    // let install = false;
    if missing_dependencies.len() >= 1 {
        cliclack::note("Missing dependencies!", 
            missing_dependencies.iter().fold("".to_string(), |acc, e| format!("{acc} {e}"))
        );
        // install = cliclack::confirm("Install missing dependencies?").interact()?;
    }

    //     for missing_dependency.
    // }
    
    // ------------------------------------------------------ 

    let project_options = ProjectOptions {
        name: project_name.clone(),
        path: cwd.join(project_name),
        template: template,
        addition_list: selected_additions,
    };

    initialize_project(&project_options)?;

    // ------------------------------------------------------ 

    let next_steps = format!(
        "cd ./{path}\nncl run dev\n",
        path = sanitize(&project_options.name)
    );

    cliclack::note("Next steps.", next_steps)?;
    cliclack::outro("Sucessfully initialized the project!")?;

    Ok(())
}

pub struct ProjectOptions {
    pub name: String,
    pub path: std::path::PathBuf,
    pub template: Template,
    pub addition_list: Vec<Addition>,
    // pub dependencies: Vec<Dependency>,
}

pub fn initialize_project(project_options: &ProjectOptions) -> std::io::Result<()> {
    if !fs::exists(&project_options.path)? {
        fs::create_dir(&project_options.path)?;
    }

    // make .data folder inside project dir
    fs::create_dir(project_options.path.join(".data"))?;

    // install selected template
    // project_options.template.install();

    // install selected additions
    for addition in project_options.addition_list.iter() {
        // addition.install();
    }

    Ok(())
}
