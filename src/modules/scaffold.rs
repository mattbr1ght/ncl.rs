use regex::Regex;
use serde::Serialize;

use crate::modules::templates::run_hook;

use super::templates::Template;
use super::common::Installable;
use std::collections::HashMap;
use std::fs;

use keyring::Entry;

#[derive(Serialize)]
pub struct ProjectOptions {
    pub name: String,
    #[serde(skip_serializing)]
    pub path: std::path::PathBuf,
    #[serde(skip_serializing)]
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
        std::fs::write(self.path.join("ncl_project_options.toml"), toml::to_string(&self).expect("Should be able to serialize ProjectOptions"))?;
        
        // post install hook
        run_hook(&self.template.post_install_hook, &self.path)?;

        // git init
        use git2;
        let mut repo: Option<git2::Repository> = None;
        if !self.path.join(".git").exists() {
            std::fs::create_dir_all(&self.path)?;
            repo = Some(git2::Repository::init(&self.path).expect("Git repository initialization should be successful"));
        }

        // create git remote repo
        let service = "ncl-cli";
        let user = whoami::username(); // or some fixed username
        let entry = Entry::new(service, &user).expect("Probably safe username should not error");

        let token_result = entry.get_password();
        let token;

        fn ask_for_token() -> String {
            cliclack::input("Please provide a github access token:")
                .required(true)
                .validate_interactively(|input: &String| {
                    if Regex::new(r"^ghp_[a-zA-Z0-9]{36}$").unwrap().is_match(input) {
                        Ok(())
                    } else {
                        Err("Not a valid personal access token")
                    }
                })
                .interact().expect("Should be fine")
        }

        match token_result {
            Ok(t) => token = t,
            Err(keyring::Error::NoEntry) => token = ask_for_token(),
            Err(_) => {panic!("Ambiguous entries for the github token in keystore")},
            // Err(keyring::Error::Ambiguous) => {}
        }

        println!("{token}");

        let client = reqwest::blocking::Client::new();
        let resp = client.post("https://api.github.com/user/repos")
            .header("User-Agent", "NCL-CLI")
            .bearer_auth(token)
            .json(&serde_json::json!({
                "name": self.name,
                "private": true
            }))
            .send();

        match resp {
            Ok(r) => {
                cliclack::log::info("Succesfully created remote repository")?;
                #[derive(serde::Deserialize)]
                struct Resp {
                    remote_url: String,
                }
                let resp: Resp = r.json().unwrap();
                repo.unwrap().remote("origin", resp.remote_url.as_str()).expect("Could not add remote origin to repo");

            },
            Err(_) => cliclack::log::error("Request to create a remote repository failed")?,
        }

        

        Ok(())
    }
}
