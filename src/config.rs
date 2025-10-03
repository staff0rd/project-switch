use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCommand {
    pub key: String,
    pub url: Option<String>,
    pub browser: Option<String>,
    pub args: Option<String>,
    #[serde(default)]
    pub url_encode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: Option<String>,
    pub description: Option<String>,
    pub browser: Option<String>,
    pub commands: Option<Vec<ProjectCommand>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "currentProject")]
    pub current_project: Option<String>,
    #[serde(rename = "defaultBrowser")]
    pub default_browser: Option<String>,
    pub global: Option<Vec<ProjectCommand>>,
    pub projects: Vec<Project>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            current_project: None,
            default_browser: None,
            global: None,
            projects: Vec::new(),
        }
    }
}

pub struct ConfigManager {
    config: Config,
    config_path: PathBuf,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_path = dirs::home_dir()
            .context("Unable to determine home directory")?
            .join(".project-switch.yml");
        
        let config = Self::load_config(&config_path)?;
        
        Ok(Self { config, config_path })
    }

    fn load_config(path: &PathBuf) -> Result<Config> {
        if path.exists() {
            let contents = fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;
            
            let config: Config = serde_yaml::from_str(&contents)
                .context("Failed to parse config file")?;
            
            Ok(config)
        } else {
            Ok(Config::default())
        }
    }

    fn save_config(&self) -> Result<()> {
        let yaml = serde_yaml::to_string(&self.config)
            .context("Failed to serialize config")?;
        
        fs::write(&self.config_path, yaml)
            .with_context(|| format!("Failed to write config file: {}", self.config_path.display()))?;
        
        Ok(())
    }

    pub fn get_projects(&self) -> &Vec<Project> {
        &self.config.projects
    }

    pub fn get_current_project(&self) -> Option<&String> {
        self.config.current_project.as_ref()
    }

    pub fn set_current_project(&mut self, project_name: &str) -> Result<()> {
        if !self.project_exists(project_name) {
            anyhow::bail!("Project '{}' not found", project_name);
        }
        
        self.config.current_project = Some(project_name.to_string());
        self.save_config()?;
        Ok(())
    }

    pub fn add_project(&mut self, project: Project) -> Result<()> {
        if self.project_exists(&project.name) {
            anyhow::bail!("Project '{}' already exists", project.name);
        }
        
        let is_first_project = self.config.projects.is_empty();
        self.config.projects.push(project.clone());
        
        if is_first_project {
            self.config.current_project = Some(project.name);
        }
        
        self.save_config()?;
        Ok(())
    }

    pub fn project_exists(&self, name: &str) -> bool {
        self.config.projects.iter().any(|p| p.name == name)
    }

    pub fn get_project(&self, name: &str) -> Option<&Project> {
        self.config.projects.iter().find(|p| p.name == name)
    }

    pub fn get_project_command(&self, project_name: &str, command_key: &str) -> Option<&ProjectCommand> {
        // First check project-specific commands
        if let Some(project_command) = self.get_project(project_name)?
            .commands
            .as_ref()
            .and_then(|cmds| cmds.iter().find(|c| c.key == command_key))
        {
            return Some(project_command);
        }
        
        // Fall back to global commands
        self.config.global
            .as_ref()
            .and_then(|cmds| cmds.iter().find(|c| c.key == command_key))
    }

    pub fn get_default_browser(&self) -> &str {
        self.config.default_browser.as_deref().unwrap_or("firefox")
    }

    pub fn get_global_commands(&self) -> Option<&Vec<ProjectCommand>> {
        self.config.global.as_ref()
    }
}