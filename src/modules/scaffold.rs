use super::templates::Template;
use super::common::Installable;
use std::collections::HashMap;
use std::fs;

pub struct ProjectOptions {
    pub name: String,
    pub path: std::path::PathBuf,
    pub template: Template,
}

impl ProjectOptions {
    pub fn vars(&self) -> HashMap<&str, &str> {
        let mut map = HashMap::new();
        map.insert("project_name", self.name.as_str());
        map.insert("path", self.path.to_str().unwrap());
        map
    }

    pub fn initialize_project(&self) -> std::io::Result<()> {
        if !fs::exists(&self.path)? {
            fs::create_dir(&self.path)?;
        }

        // maybe move universal template out of templates/ folder to eliminate possible name
        // conflicts
        use super::templates::load_templates;
        let base_template = load_templates()?.into_iter().find(|template| template.name == "Universal Base");
        match base_template {
            Some(base_template) => base_template.install(&self)?,
            None => {}
        };

        // install selected template
        self.template.install(&self)?;

        // git init

        Ok(())
    }
}
