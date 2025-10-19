use crate::modules::scaffold::ProjectOptions;
use crate::modules::common::{Installable, ncl_config_dir, Dependency};

use serde::Deserialize;

use std::io::{Seek, Read};
use std::path::PathBuf;
use include_dir::{include_dir, Dir};
use std::{collections::HashMap, fs::File};

// ------------------ Include Templates in Bin ------------------ //

static DEFAULT_TEMPLATES: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");

pub fn write_default_templates(target_path: &std::path::Path) -> std::io::Result<()> {
    DEFAULT_TEMPLATES.extract(target_path)?;
    Ok(())
}

// -------------------------------------------------------------- //

#[derive(Deserialize)]
#[derive(Debug)]
#[derive(Clone, Eq, PartialEq)]
#[serde(default = "Template::default")]
pub struct Template {
    pub name: String,
    pub path: std::path::PathBuf,
    pub comment: String,
    pub dependencies: Vec<Dependency>,
}


impl Template {
    pub fn default() -> Self {
        Self {
            name: String::new(),
            path: std::path::PathBuf::new(),
            comment: String::new(),
            dependencies: vec![],
        }
    }

    #[allow(dead_code)]
    pub fn new(name: String, path: String, comment: String, dependencies: Vec<Dependency>) -> Self {
        Self {
            name: name,
            path: std::path::PathBuf::from(path),
            comment: comment,
            dependencies: dependencies,
        }
    }

}

pub fn load_templates() -> std::io::Result<Vec<Template>> {
    let mut templates = Vec::new();

    ensure_templates_exist()?;

    for entry in std::fs::read_dir(templates_dir())? {
        let path = entry?.path();
        if path.is_dir() && path.join("template.toml").exists() {
            let meta = std::fs::read_to_string(path.join("template.toml"))?;
            let mut template: Template = toml::from_str(&meta).expect("Toml deserialization error");
            template.path = path;
            templates.push(template);
        }
    }
    Ok(templates)
}


impl Installable for Template {
    fn install(&self, project_options: &ProjectOptions) -> std::io::Result<()> {
        copy_dir_recursive(&self.path, &project_options.path)?;

        let vars = project_options.vars();
        replace_placeholders_recursively(&project_options.path, &vars)?;

        Ok(())
    }
}

// -------------------------------------------------------------- //

fn templates_dir() -> PathBuf {
    ncl_config_dir().join("templates")
}

fn ensure_templates_exist() -> std::io::Result<()> {
    let dir = templates_dir();

    if !dir.exists() {
        write_default_templates(&dir)?;
    }

    Ok(())
}

fn copy_dir_recursive(src: impl AsRef<std::path::Path>, dst: impl AsRef<std::path::Path>) -> std::io::Result<()> {
    std::fs::create_dir_all(&dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_recursive(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            std::fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

fn replace_placeholders_recursively(root: &std::path::Path, vars: &HashMap<&str, &str>) -> std::io::Result<()> {
    for entry in walkdir::WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path.is_file() && is_text_file(path) {
            let mut text = std::fs::read_to_string(path)?;
            for (key, val) in vars {
                let pattern = format!("{{{{{}}}}}", key);
                text = text.replace(&pattern, val);
            }
            std::fs::write(path, text)?;
        }
    }
    Ok(())
}

fn is_text_file(path: &std::path::Path) -> bool {
    let start = 0;
    let count;
    let mut f = File::open(path).expect("Should be a valid file path");
    if f.metadata().expect("Should be a valid file to read").len() >= 8000{
        count = 8000;
    } else {
        count = f.metadata().expect("Should be a valid file to read").len();
    }
    f.seek(std::io::SeekFrom::Start(start)).expect("Should succesfully seek the file");
    let mut buf = vec![0; count as usize];
    f.read_exact(&mut buf).expect("Should sucessfully read {count} bytes from file");
    !buf.contains(&0)
}
