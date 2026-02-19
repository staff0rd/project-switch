use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::fs;
use std::path::PathBuf;

// Helper function to skip serializing false values
fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShortcutsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(rename = "extraPaths", skip_serializing_if = "Option::is_none")]
    pub extra_paths: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

fn default_true() -> bool {
    true
}

impl Default for ShortcutsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            extra_paths: None,
            exclude: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectCommand {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub url_encode: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commands: Option<Vec<ProjectCommand>>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(rename = "currentProject", skip_serializing_if = "Option::is_none")]
    pub current_project: Option<String>,
    #[serde(rename = "defaultBrowser", skip_serializing_if = "Option::is_none")]
    pub default_browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<Vec<ProjectCommand>>,
    #[serde(default)]
    pub shortcuts: Option<ShortcutsConfig>,
    pub projects: Vec<Project>,
}

pub struct ConfigManager {
    config: Config,
    config_path: PathBuf,
    raw_yaml: Option<Value>,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_path = dirs::home_dir()
            .context("Unable to determine home directory")?
            .join(".project-switch.yml");

        let (config, raw_yaml) = Self::load_config(&config_path)?;

        Ok(Self {
            config,
            config_path,
            raw_yaml,
        })
    }

    fn load_config(path: &PathBuf) -> Result<(Config, Option<Value>)> {
        if path.exists() {
            let contents = fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;

            let config: Config =
                serde_yaml::from_str(&contents).context("Failed to parse config file")?;

            let raw_yaml: Value = serde_yaml::from_str(&contents)
                .context("Failed to parse config file as raw YAML")?;

            Ok((config, Some(raw_yaml)))
        } else {
            Ok((Config::default(), None))
        }
    }

    fn save_config(&mut self) -> Result<()> {
        let yaml_value = if let Some(ref mut raw) = self.raw_yaml {
            // Update the raw YAML with current config values while preserving order
            if let Value::Mapping(ref mut map) = raw {
                // Update currentProject
                let current_project_key = Value::String("currentProject".to_string());
                if let Some(ref current) = self.config.current_project {
                    map.insert(current_project_key, Value::String(current.clone()));
                } else {
                    map.remove(&current_project_key);
                }

                // Update projects array
                let projects_key = Value::String("projects".to_string());
                let projects_value = serde_yaml::to_value(&self.config.projects)
                    .context("Failed to serialize projects")?;
                map.insert(projects_key, projects_value);
            }
            raw.clone()
        } else {
            // No existing file, serialize the whole config
            serde_yaml::to_value(&self.config).context("Failed to serialize config")?
        };

        let yaml = serde_yaml::to_string(&yaml_value).context("Failed to serialize config")?;

        fs::write(&self.config_path, yaml).with_context(|| {
            format!(
                "Failed to write config file: {}",
                self.config_path.display()
            )
        })?;

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

    /// Returns the current project name and its configuration, or None if not set/found.
    pub fn resolve_current_project(&self) -> Option<(&String, &Project)> {
        let name = self.get_current_project()?;
        self.get_project(name).map(|p| (name, p))
    }

    pub fn get_project_command(
        &self,
        project_name: &str,
        command_key: &str,
    ) -> Option<&ProjectCommand> {
        // First check project-specific commands
        if let Some(project_command) = self
            .get_project(project_name)?
            .commands
            .as_ref()
            .and_then(|cmds| cmds.iter().find(|c| c.key == command_key))
        {
            return Some(project_command);
        }

        // Fall back to global commands
        self.config
            .global
            .as_ref()
            .and_then(|cmds| cmds.iter().find(|c| c.key == command_key))
    }

    pub fn get_default_browser(&self) -> &str {
        self.config.default_browser.as_deref().unwrap_or("firefox")
    }

    pub fn get_global_commands(&self) -> Option<&Vec<ProjectCommand>> {
        self.config.global.as_ref()
    }

    pub fn get_shortcuts_config(&self) -> ShortcutsConfig {
        self.config.shortcuts.clone().unwrap_or_default()
    }
}
