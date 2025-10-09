pub trait Installable {
     fn install(&self) -> std::io::Result<()>;
}

#[derive(Clone, Eq, PartialEq)]
pub enum Addition {
    GitHubRepo,
    Prettier,
}

#[derive(Clone, Eq, PartialEq)]
pub struct Additional {
    pub name: String,
    pub addition: Addition,
    pub comment: String,
}

impl Additional {
    pub fn new(name: &str, addition: Addition, comment: &str) -> Self {
        Self {
            name: name.to_string(),
            addition: addition,
            comment: comment.to_string(),
        }
    }
}

impl Installable for Additional {
    fn install(&self) -> std::io::Result<()> {
        Ok(())
    }
}


#[derive(Clone, Eq, PartialEq)]
pub struct Template {
    pub name: String,
    pub path: std::path::PathBuf,
    pub comment: String,
    pub dependencies: Vec<String>,
}

impl Installable for Template {
    fn install(&self) -> std::io::Result<()> {
        // std::env::current_dir()?
        Ok(())
    }
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

    pub fn new(name: &str, path: &str, comment: &str, dependencies: Vec<String>) -> Self {
        Self {
            name: name.to_string(),
            path: std::path::PathBuf::from(path),
            comment: comment.to_string(),
            dependencies: dependencies,
        }
    }

}

