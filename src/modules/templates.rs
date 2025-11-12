use anyhow::{Context, Result};
use log::debug;
use crate::modules::scaffold::ProjectOptions;
use crate::modules::common::{Installable, ncl_config_dir, Dependency};
use crate::modules::execution::execute_commands;

use serde::Deserialize;
use std::collections::HashMap;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};
use include_dir::{include_dir, Dir};

static DEFAULT_TEMPLATES: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");

/// A hook is a sequence of shell commands to execute
pub type Hook = Vec<String>;

/// Represents a project template with its configuration
#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(default)]
pub struct Template {
    pub name: String,
    pub path: PathBuf,
    pub comment: String,
    pub dependencies: Vec<Dependency>,
    /// Post-install hook (deprecated, use hooks instead)
    #[serde(default)]
    pub post_install_hook: Hook,
    /// Named hooks for different execution contexts
    #[serde(default)]
    pub hooks: HashMap<String, Hook>,
    pub jobs: Option<HashMap<String, Vec<String>>>,
}

impl Default for Template {
    fn default() -> Self {
        Self {
            name: String::new(),
            path: PathBuf::new(),
            comment: String::new(),
            dependencies: vec![],
            post_install_hook: vec![],
            hooks: HashMap::new(),
            jobs: None,
        }
    }
}

impl Template {
    /// Get the post-install hook, checking both old and new formats
    pub fn get_post_install_hook(&self) -> &Hook {
        // Prefer new hooks format, fallback to old post_install_hook
        self.hooks.get("post_install")
            .or_else(|| if !self.post_install_hook.is_empty() {
                Some(&self.post_install_hook)
            } else {
                None
            })
            .unwrap_or(&self.post_install_hook)
    }

    /// Check if this is the universal base template
    pub fn is_universal_base(&self) -> bool {
        self.name == "Universal Base"
    }
}

/// Loads all available templates from the templates directory (excluding universal base)
pub fn load_templates() -> Result<Vec<Template>> {
    ensure_templates_exist()?;

    let templates: Vec<Template> = std::fs::read_dir(templates_dir())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path.join("template.toml").exists())
        .filter_map(|path| load_template_from_path(&path).ok())
        .filter(|template| !template.is_universal_base())
        .collect();
    
    Ok(templates)
}

/// Loads the universal base template from its separate directory
pub fn load_universal_base() -> Result<Option<Template>> {
    let universal_path = universal_base_dir();
    
    if !universal_path.exists() {
        // Try to extract it from default templates
        ensure_universal_base_exists()?;
    }
    
    if !universal_path.join("template.toml").exists() {
        return Ok(None);
    }
    
    load_template_from_path(&universal_path)
        .map(Some)
        .or_else(|e| {
            debug!("Failed to load universal base: {}", e);
            Ok(None)
        })
}

fn ensure_universal_base_exists() -> Result<()> {
    let universal_path = universal_base_dir();
    
    if universal_path.exists() && universal_path.join("template.toml").exists() {
        return Ok(());
    }
    
    // Extract universal base from default templates
    let templates_path = templates_dir();
    ensure_templates_exist()?;
    
    // Look for universal-base in the templates directory and copy it
    let source_path = templates_path.join("universal-base");
    if source_path.exists() && source_path.join("template.toml").exists() {
        std::fs::create_dir_all(&universal_path)
            .with_context(|| format!("Failed to create universal-base directory: {}", universal_path.display()))?;
        
        copy_dir_recursive(&source_path, &universal_path)
            .with_context(|| "Failed to copy universal-base template")?;
    }
    
    Ok(())
}

fn load_template_from_path(path: &Path) -> Result<Template> {
    let meta = std::fs::read_to_string(path.join("template.toml"))
        .with_context(|| format!("Failed to read template.toml at {}", path.display()))?;
    
    let mut template: Template = toml::from_str(&meta)
        .with_context(|| format!("Failed to parse template.toml at {}", path.display()))?;
    template.path = path.to_path_buf();
    Ok(template)
}


impl Installable for Template {
    fn install(&self, project_options: &ProjectOptions) -> Result<()> {
        copy_dir_recursive(&self.path, &project_options.path)
            .with_context(|| format!("Failed to copy template '{}'", self.name))?;

        let vars = project_options.vars();
        replace_placeholders_recursively(&project_options.path, &vars)
            .context("Failed to replace placeholders in template files")?;

        Ok(())
    }
}

/// Executes a hook (sequence of commands) in the project directory
pub fn run_hook(hook: &Hook, project_path: &Path) -> Result<()> {
    execute_commands(hook, project_path, "Running hook")
}

/// Executes a named hook from a template
pub fn run_named_hook(template: &Template, hook_name: &str, project_path: &Path) -> Result<()> {
    let hook = template.hooks.get(hook_name)
        .with_context(|| format!("Hook '{}' not found in template '{}'", hook_name, template.name))?;
    
    execute_commands(hook, project_path, &format!("Running hook: {}", hook_name))
}

fn templates_dir() -> PathBuf {
    ncl_config_dir().join("templates")
}

/// Returns the path to the universal base template directory (separate from templates)
pub fn universal_base_dir() -> PathBuf {
    ncl_config_dir().join("universal-base")
}

fn ensure_templates_exist() -> Result<()> {
    let dir = templates_dir();
    if !dir.exists() {
        write_default_templates(&dir)?;
    }
    Ok(())
}

/// Writes default templates to the target path
pub fn write_default_templates(target_path: &Path) -> Result<()> {
    std::fs::create_dir_all(target_path)
        .with_context(|| format!("Failed to create templates directory: {}", target_path.display()))?;
    DEFAULT_TEMPLATES.extract(target_path)
        .with_context(|| format!("Failed to extract default templates to {}", target_path.display()))?;
    
    // After extracting, move universal-base to its separate directory
    let universal_source = target_path.join("universal-base");
    if universal_source.exists() {
        let universal_dest = universal_base_dir();
        std::fs::create_dir_all(universal_dest.parent().unwrap())?;
        
        // Copy universal-base to separate location
        if !universal_dest.exists() {
            copy_dir_recursive(&universal_source, &universal_dest)
                .context("Failed to copy universal-base to separate directory")?;
        }
        
        // Remove universal-base from templates directory
        if universal_source.exists() {
            std::fs::remove_dir_all(&universal_source)
                .context("Failed to remove universal-base from templates directory")?;
        }
    }
    
    Ok(())
}

fn copy_dir_recursive(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> Result<()> {
    let src_path = src.as_ref();
    let dst_path = dst.as_ref();
    
    std::fs::create_dir_all(dst_path)
        .with_context(|| format!("Failed to create directory: {}", dst_path.display()))?;
    
    for entry in std::fs::read_dir(src_path)
        .with_context(|| format!("Failed to read directory: {}", src_path.display()))? {
        let entry = entry?;
        let dst_path = dst_path.join(entry.file_name());
        
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(entry.path(), dst_path)?;
        } else {
            std::fs::copy(entry.path(), &dst_path)
                .with_context(|| format!("Failed to copy file to: {}", dst_path.display()))?;
        }
    }
    
    Ok(())
}

fn replace_placeholders_recursively(root: &Path, vars: &HashMap<&str, &str>) -> Result<()> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.path().to_path_buf())
        .filter(|path| path.is_file() && is_text_file(path))
        .try_for_each(|path| replace_placeholders_in_file(&path, vars))
        .context("Failed to replace placeholders in files")
}

fn replace_placeholders_in_file(path: &Path, vars: &HashMap<&str, &str>) -> Result<()> {
    let mut text = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;
    
    for (key, val) in vars {
        let pattern = format!("{{{{{}}}}}", key);
        text = text.replace(&pattern, val);
    }
    
    std::fs::write(path, text)
        .with_context(|| format!("Failed to write file: {}", path.display()))?;
    Ok(())
}

fn is_text_file(path: &Path) -> bool {
    use std::fs::File;
    
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    
    let Ok(metadata) = file.metadata() else {
        return false;
    };
    
    let sample_size = metadata.len().min(8000) as usize;
    let mut buf = vec![0; sample_size];
    
    if file.seek(std::io::SeekFrom::Start(0)).is_err() {
        return false;
    }
    
    if file.read_exact(&mut buf).is_err() {
        return false;
    }
    
    !buf.contains(&0)
}
