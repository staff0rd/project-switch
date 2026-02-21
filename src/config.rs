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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(rename = "currentProject", skip_serializing_if = "Option::is_none")]
    pub current_project: Option<String>,
    #[serde(rename = "defaultBrowser", skip_serializing_if = "Option::is_none")]
    pub default_browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<Vec<ProjectCommand>>,
    #[serde(default)]
    pub shortcuts: Option<ShortcutsConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub projects: Vec<Project>,
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn merge_configs(base: Config, overlay: Config) -> Config {
    Config {
        include: overlay.include,
        current_project: overlay.current_project.or(base.current_project),
        default_browser: overlay.default_browser.or(base.default_browser),
        global: merge_command_lists(base.global, overlay.global),
        // shortcuts is machine-specific: local replaces entirely
        shortcuts: if overlay.shortcuts.is_some() {
            overlay.shortcuts
        } else {
            base.shortcuts
        },
        projects: merge_project_lists(base.projects, overlay.projects),
    }
}

fn merge_project_lists(base: Vec<Project>, overlay: Vec<Project>) -> Vec<Project> {
    let mut merged: Vec<Project> = Vec::new();

    // Start with base projects, merging overlay matches
    for base_proj in &base {
        if let Some(overlay_proj) = overlay.iter().find(|p| p.name == base_proj.name) {
            merged.push(merge_projects(base_proj.clone(), overlay_proj.clone()));
        } else {
            merged.push(base_proj.clone());
        }
    }

    // Append overlay-only projects (not in base), sorted by name
    let mut overlay_only: Vec<Project> = overlay
        .into_iter()
        .filter(|p| !base.iter().any(|b| b.name == p.name))
        .collect();
    overlay_only.sort_by(|a, b| a.name.cmp(&b.name));
    merged.extend(overlay_only);

    merged
}

fn merge_projects(base: Project, overlay: Project) -> Project {
    Project {
        name: overlay.name,
        path: overlay.path.or(base.path),
        description: overlay.description.or(base.description),
        browser: overlay.browser.or(base.browser),
        commands: merge_command_lists(base.commands, overlay.commands),
    }
}

fn merge_command_lists(
    base: Option<Vec<ProjectCommand>>,
    overlay: Option<Vec<ProjectCommand>>,
) -> Option<Vec<ProjectCommand>> {
    match (base, overlay) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(o)) => Some(o),
        (Some(base_cmds), Some(overlay_cmds)) => {
            let mut merged: Vec<ProjectCommand> = Vec::new();

            for base_cmd in &base_cmds {
                if let Some(overlay_cmd) = overlay_cmds.iter().find(|c| c.key == base_cmd.key) {
                    merged.push(merge_commands(base_cmd.clone(), overlay_cmd.clone()));
                } else {
                    merged.push(base_cmd.clone());
                }
            }

            // Append overlay-only commands
            let overlay_only: Vec<ProjectCommand> = overlay_cmds
                .into_iter()
                .filter(|c| !base_cmds.iter().any(|b| b.key == c.key))
                .collect();
            merged.extend(overlay_only);

            Some(merged)
        }
    }
}

fn merge_commands(base: ProjectCommand, overlay: ProjectCommand) -> ProjectCommand {
    ProjectCommand {
        key: overlay.key,
        url: overlay.url.or(base.url),
        browser: overlay.browser.or(base.browser),
        args: overlay.args.or(base.args),
        url_encode: base.url_encode || overlay.url_encode,
    }
}

pub struct ConfigManager {
    config: Config,
    config_path: PathBuf,
    raw_yaml: Option<Value>,
    local_projects: Vec<Project>,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_path = dirs::home_dir()
            .context("Unable to determine home directory")?
            .join(".project-switch.yml");

        let (config, raw_yaml, local_projects) = Self::load_config(&config_path)?;

        Ok(Self {
            config,
            config_path,
            raw_yaml,
            local_projects,
        })
    }

    fn load_config(path: &PathBuf) -> Result<(Config, Option<Value>, Vec<Project>)> {
        if path.exists() {
            let contents = fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;

            let local_config: Config =
                serde_yaml::from_str(&contents).context("Failed to parse config file")?;

            let raw_yaml: Value = serde_yaml::from_str(&contents)
                .context("Failed to parse config file as raw YAML")?;

            let local_projects = local_config.projects.clone();

            // Handle include
            let config = if let Some(ref include_path) = local_config.include {
                let resolved = expand_tilde(include_path);
                if resolved.exists() {
                    let base_contents = fs::read_to_string(&resolved).with_context(|| {
                        format!("Failed to read included config: {}", resolved.display())
                    })?;
                    let base_config: Config =
                        serde_yaml::from_str(&base_contents).with_context(|| {
                            format!("Failed to parse included config: {}", resolved.display())
                        })?;
                    merge_configs(base_config, local_config)
                } else {
                    eprintln!("Warning: included config not found: {}", resolved.display());
                    local_config
                }
            } else {
                local_config
            };

            Ok((config, Some(raw_yaml), local_projects))
        } else {
            let default_contents = "projects: []\n";
            fs::write(path, default_contents).with_context(|| {
                format!("Failed to create default config file: {}", path.display())
            })?;
            let config: Config =
                serde_yaml::from_str(default_contents).context("Failed to parse default config")?;
            let raw_yaml: Value = serde_yaml::from_str(default_contents)
                .context("Failed to parse default config as raw YAML")?;
            Ok((config, Some(raw_yaml), Vec::new()))
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

                // Update projects array (only local projects, not merged)
                let projects_key = Value::String("projects".to_string());
                let projects_value = serde_yaml::to_value(&self.local_projects)
                    .context("Failed to serialize projects")?;
                map.insert(projects_key, projects_value);
            }
            raw.clone()
        } else {
            // No existing file, serialize a config with only local projects
            let local_config = Config {
                include: self.config.include.clone(),
                current_project: self.config.current_project.clone(),
                default_browser: self.config.default_browser.clone(),
                global: self.config.global.clone(),
                shortcuts: self.config.shortcuts.clone(),
                projects: self.local_projects.clone(),
            };
            serde_yaml::to_value(&local_config).context("Failed to serialize config")?
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
        self.local_projects.push(project.clone());

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
