use std::fs;
use console::style;
use sanitize_filename::is_sanitized;
use sanitize_filename::sanitize;

pub fn run() -> std::io::Result<()>{

    cliclack::clear_screen()?;
    cliclack::intro(style(" init ").on_green().black())?;

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


    let _kind = cliclack::select(format!("Pick a project type within '{project_name}'"))
        .initial_value("ts")
        .item("ts", "TypeScript", "")
        .item("js", "JavaScript", "")
        .item("coffee", "CoffeeScript", "oh no")
        .interact()?;

    let _tools = cliclack::multiselect("Select additional tools")
        .initial_values(vec!["prettier", "eslint"])
        .item("prettier", "Prettier", "recommended")
        .item("eslint", "ESLint", "recommended")
        .item("stylelint", "Stylelint", "")
        .item("gh-action", "GitHub Action", "")
        .interact()?;

    let next_steps = format!(
        "cd ./{path}\nncl run dev\n",
        path = sanitize(project_name)
    );

    cliclack::note("Next steps.", next_steps)?;
    cliclack::outro("Sucessfully initialized the project!")?;

    Ok(())
}
