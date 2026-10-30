use crate::launcher::key_combo::KeyCombo;
use crate::launcher::CALENDAR_HOTKEY;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_yaml::Value;
use std::fs;
use std::path::{Path, PathBuf};

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

fn is_false(b: &bool) -> bool {
    !*b
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

/// Tray-managed WSL assist webserver settings. Owned and written by the
/// project-switch-hotkey tray app; the CLI only needs to accept the section so
/// it round-trips without tripping `deny_unknown_fields`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebserverConfig {
    #[serde(default, skip_serializing_if = "is_false")]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distro: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

/// One entry of the tray-managed `webservers` list; like [`WebserverConfig`],
/// the CLI only round-trips it.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebserverEntryConfig {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    #[serde(flatten)]
    pub settings: WebserverConfig,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PaprikaConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarConfig {
    #[serde(rename = "clientId", default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    #[serde(
        rename = "clientSecret",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub client_secret: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub accounts: Vec<CalendarAccount>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CalendarAccount {
    pub label: String,
    #[serde(
        rename = "refreshToken",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub refresh_token: Option<String>,
}

fn null_as_empty_section<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Ok(Some(
        Option::<T>::deserialize(deserializer)?.unwrap_or_default(),
    ))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserOption {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CommandBrowser {
    Single(String),
    Choice(Vec<BrowserOption>),
}

impl CommandBrowser {
    pub fn first_value(&self) -> &str {
        match self {
            CommandBrowser::Single(value) => value,
            CommandBrowser::Choice(options) => &options[0].value,
        }
    }

    pub fn choices(&self) -> Option<&[BrowserOption]> {
        match self {
            CommandBrowser::Choice(options) if options.len() > 1 => Some(options),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectCommand {
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browser: Option<CommandBrowser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub args: Option<String>,
    /// Open this command's URL in the reusable borderless webview window
    /// instead of a browser. Mutually exclusive with `command`.
    #[serde(default, skip_serializing_if = "is_false")]
    pub webview: bool,
    /// When the webview window is already open, send it this command's URL
    /// instead of only bringing it to the front.
    #[serde(default, skip_serializing_if = "is_false")]
    pub navigate: bool,
    /// Open the webview window maximised when this command spawns it.
    #[serde(default, skip_serializing_if = "is_false")]
    pub maximized: bool,
    /// Force this command to the top of the recent list when the launcher
    /// opens with empty input, regardless of when it was last used.
    #[serde(default, skip_serializing_if = "is_false")]
    pub pinned: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
}

impl ProjectCommand {
    pub fn key_combo(&self) -> Option<KeyCombo> {
        self.hotkey.as_deref()?.parse().ok()
    }
}

/// Declares an entry struct carrying the schema shared by clients and projects
/// (`name`, `path`, `description`, `browser`, `commands`), plus any
/// struct-specific fields listed in the body.
macro_rules! entry_struct {
    ($name:ident { $($extra:tt)* }) => {
        #[derive(Debug, Clone, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            pub name: String,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub path: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub description: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub browser: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub commands: Option<Vec<ProjectCommand>>,
            $($extra)*
        }
    };
}

entry_struct!(Project {});
entry_struct!(Client {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub projects: Option<Vec<Project>>,
});

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub include: Option<String>,
    #[serde(rename = "currentClient", skip_serializing_if = "Option::is_none")]
    pub current_client: Option<String>,
    #[serde(rename = "currentProject", skip_serializing_if = "Option::is_none")]
    pub current_project: Option<String>,
    #[serde(rename = "defaultBrowser", skip_serializing_if = "Option::is_none")]
    pub default_browser: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub global: Option<Vec<ProjectCommand>>,
    #[serde(default)]
    pub shortcuts: Option<ShortcutsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webserver: Option<WebserverConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub webservers: Option<Vec<WebserverEntryConfig>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub monitor: Option<u32>,
    #[serde(
        default,
        deserialize_with = "null_as_empty_section",
        skip_serializing_if = "Option::is_none"
    )]
    pub paprika: Option<PaprikaConfig>,
    #[serde(
        default,
        deserialize_with = "null_as_empty_section",
        skip_serializing_if = "Option::is_none"
    )]
    pub calendar: Option<CalendarConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clients: Vec<Client>,
}

/// Result of [`ConfigManager::resolve_current`]: the active client and,
/// when the nested-project key is set and valid, the selected project.
pub type ResolvedSelection<'a> = (&'a String, &'a Client, Option<(&'a String, &'a Project)>);

/// Expand tilde in an include path to the home directory.
pub fn expand_include_path(path: &str) -> PathBuf {
    expand_tilde(path)
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/").or_else(|| path.strip_prefix("~\\")) {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

/// Rewrite an old-schema YAML document (`projects:` / `currentProject:`) to the new
/// schema (`clients:` / `currentClient:`). Returns the migrated string and whether
/// a migration was actually performed.
///
/// The trigger is top-level `projects:`. In the new schema `currentProject` is a
/// valid key (the nested project selection), so it must not be touched once the
/// config has already been migrated to `clients:`.
fn migrate_schema(contents: &str) -> Result<(String, bool)> {
    let doc: Value =
        serde_yaml::from_str(contents).context("Failed to parse YAML for migration")?;

    let Value::Mapping(map) = doc else {
        return Ok((contents.to_string(), false));
    };

    if !map.contains_key(Value::String("projects".to_string())) {
        return Ok((contents.to_string(), false));
    }

    let mut new_map = serde_yaml::Mapping::new();
    for (key, value) in map {
        let new_key = match key {
            Value::String(ref s) if s == "projects" => Value::String("clients".to_string()),
            Value::String(ref s) if s == "currentProject" => {
                Value::String("currentClient".to_string())
            }
            other => other,
        };
        new_map.insert(new_key, value);
    }

    let new_contents = serde_yaml::to_string(&Value::Mapping(new_map))
        .context("Failed to serialize migrated YAML")?;
    Ok((new_contents, true))
}

/// Read a config file, running the schema migration in place when old keys are present.
fn read_and_migrate(path: &Path) -> Result<String> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let (migrated_contents, did_migrate) = migrate_schema(&contents)?;
    if did_migrate {
        fs::write(path, &migrated_contents)
            .with_context(|| format!("Failed to write migrated config: {}", path.display()))?;
        eprintln!(
            "Migrated config schema (projects -> clients): {}",
            path.display()
        );
    }
    Ok(migrated_contents)
}

fn merge_configs(base: Config, overlay: Config) -> Config {
    Config {
        include: overlay.include,
        current_client: overlay.current_client.or(base.current_client),
        current_project: overlay.current_project.or(base.current_project),
        default_browser: overlay.default_browser.or(base.default_browser),
        global: merge_command_lists(base.global, overlay.global),
        // shortcuts is machine-specific: local replaces entirely
        shortcuts: if overlay.shortcuts.is_some() {
            overlay.shortcuts
        } else {
            base.shortcuts
        },
        // webserver is machine-specific: local replaces entirely
        webserver: if overlay.webserver.is_some() {
            overlay.webserver
        } else {
            base.webserver
        },
        webservers: if overlay.webservers.is_some() {
            overlay.webservers
        } else {
            base.webservers
        },
        monitor: overlay.monitor.or(base.monitor),
        paprika: merge_paprika(base.paprika, overlay.paprika),
        calendar: merge_calendar(base.calendar, overlay.calendar),
        clients: merge_client_lists(base.clients, overlay.clients),
    }
}

fn merge_calendar(
    base: Option<CalendarConfig>,
    overlay: Option<CalendarConfig>,
) -> Option<CalendarConfig> {
    match (base, overlay) {
        (Some(b), Some(o)) => Some(CalendarConfig {
            client_id: o.client_id.or(b.client_id),
            client_secret: o.client_secret.or(b.client_secret),
            accounts: merge_keyed_lists(
                b.accounts,
                o.accounts,
                |a| a.label.as_str(),
                |b, o| CalendarAccount {
                    label: o.label,
                    refresh_token: o.refresh_token.or(b.refresh_token),
                },
                false,
            ),
        }),
        (b, o) => o.or(b),
    }
}

fn merge_paprika(
    base: Option<PaprikaConfig>,
    overlay: Option<PaprikaConfig>,
) -> Option<PaprikaConfig> {
    match (base, overlay) {
        (Some(b), Some(o)) => Some(PaprikaConfig {
            image: o.image.or(b.image),
            token: o.token.or(b.token),
        }),
        (b, o) => o.or(b),
    }
}

/// Merge two keyed lists. Entries whose key appears in both are combined via
/// `merge`; base-only entries keep their order; overlay-only entries are
/// appended afterwards (sorted by key when `sort_appended`).
fn merge_keyed_lists<T: Clone>(
    base: Vec<T>,
    overlay: Vec<T>,
    key: impl Fn(&T) -> &str,
    merge: impl Fn(T, T) -> T,
    sort_appended: bool,
) -> Vec<T> {
    let mut merged: Vec<T> = Vec::new();

    for base_entry in &base {
        if let Some(overlay_entry) = overlay.iter().find(|&o| key(o) == key(base_entry)) {
            merged.push(merge(base_entry.clone(), overlay_entry.clone()));
        } else {
            merged.push(base_entry.clone());
        }
    }

    let mut overlay_only: Vec<T> = overlay
        .into_iter()
        .filter(|o| !base.iter().any(|b| key(b) == key(o)))
        .collect();
    if sort_appended {
        overlay_only.sort_by(|a, b| key(a).cmp(key(b)));
    }
    merged.extend(overlay_only);

    merged
}

/// [`merge_keyed_lists`] lifted over the `Option<Vec<_>>` the config uses for
/// optional command/project lists: two unset lists stay unset rather than
/// collapsing to an empty list.
fn merge_optional_lists<T: Clone>(
    base: Option<Vec<T>>,
    overlay: Option<Vec<T>>,
    key: impl Fn(&T) -> &str,
    merge: impl Fn(T, T) -> T,
    sort_appended: bool,
) -> Option<Vec<T>> {
    match (base, overlay) {
        (None, None) => None,
        (Some(b), None) => Some(b),
        (None, Some(o)) => Some(o),
        (Some(b), Some(o)) => Some(merge_keyed_lists(b, o, key, merge, sort_appended)),
    }
}

fn merge_client_lists(base: Vec<Client>, overlay: Vec<Client>) -> Vec<Client> {
    merge_keyed_lists(base, overlay, |c| c.name.as_str(), merge_clients, true)
}

/// Merge the fields shared by [`Client`] and [`Project`] (everything except
/// the client-only `projects` field).
macro_rules! merge_shared_fields {
    ($base:expr, $overlay:expr) => {
        (
            $overlay.name,
            $overlay.path.or($base.path),
            $overlay.description.or($base.description),
            $overlay.browser.or($base.browser),
            merge_command_lists($base.commands, $overlay.commands),
        )
    };
}

fn merge_clients(mut base: Client, mut overlay: Client) -> Client {
    let projects = merge_project_lists(base.projects.take(), overlay.projects.take());
    let (name, path, description, browser, commands) = merge_shared_fields!(base, overlay);
    Client {
        name,
        path,
        description,
        browser,
        commands,
        projects,
    }
}

fn merge_project_lists(
    base: Option<Vec<Project>>,
    overlay: Option<Vec<Project>>,
) -> Option<Vec<Project>> {
    merge_optional_lists(base, overlay, |p| p.name.as_str(), merge_projects, true)
}

fn merge_projects(base: Project, overlay: Project) -> Project {
    let (name, path, description, browser, commands) = merge_shared_fields!(base, overlay);
    Project {
        name,
        path,
        description,
        browser,
        commands,
    }
}

fn merge_command_lists(
    base: Option<Vec<ProjectCommand>>,
    overlay: Option<Vec<ProjectCommand>>,
) -> Option<Vec<ProjectCommand>> {
    merge_optional_lists(base, overlay, |c| c.key.as_str(), merge_commands, false)
}

fn merge_commands(base: ProjectCommand, overlay: ProjectCommand) -> ProjectCommand {
    ProjectCommand {
        key: overlay.key,
        url: overlay.url.or(base.url),
        command: overlay.command.or(base.command),
        browser: overlay.browser.or(base.browser),
        args: overlay.args.or(base.args),
        webview: overlay.webview || base.webview,
        navigate: overlay.navigate || base.navigate,
        maximized: overlay.maximized || base.maximized,
        pinned: overlay.pinned || base.pinned,
        hotkey: overlay.hotkey.or(base.hotkey),
    }
}

fn validate_hotkeys(commands: &[ProjectCommand], context: &str) -> Result<()> {
    let mut seen: std::collections::HashMap<KeyCombo, &str> =
        std::collections::HashMap::from([(CALENDAR_HOTKEY, "the calendar sidebar")]);
    for cmd in commands {
        let Some(hotkey) = cmd.hotkey.as_deref() else {
            continue;
        };
        let combo: KeyCombo = hotkey
            .parse()
            .map_err(|e| anyhow::anyhow!("Command '{}' in {} has an {}", cmd.key, context, e))?;
        if let Some(other) = seen.insert(combo, &cmd.key) {
            anyhow::bail!(
                "Command '{}' in {} uses hotkey '{}', already bound to '{}'",
                cmd.key,
                context,
                combo,
                other
            );
        }
    }
    Ok(())
}

fn validate_command_list(commands: &[ProjectCommand], context: &str) -> Result<()> {
    for cmd in commands {
        if cmd.url.is_some() && cmd.command.is_some() {
            anyhow::bail!(
                "Command '{}' in {} has both 'url' and 'command' — use one or the other",
                cmd.key,
                context
            );
        }
        if cmd.command.is_some() && cmd.browser.is_some() {
            anyhow::bail!(
                "Command '{}' in {} has both 'command' and 'browser' — 'command' runs directly, not in a browser",
                cmd.key,
                context
            );
        }
        if let Some(CommandBrowser::Choice(options)) = &cmd.browser {
            if options.is_empty() {
                anyhow::bail!(
                    "Command '{}' in {} has an empty 'browser' list — give at least one {{label, value}} entry",
                    cmd.key,
                    context
                );
            }
            let mut labels = std::collections::HashSet::new();
            for option in options {
                if !labels.insert(option.label.as_str()) {
                    anyhow::bail!(
                        "Command '{}' in {} has duplicate browser label '{}'",
                        cmd.key,
                        context,
                        option.label
                    );
                }
            }
        }
        if cmd.webview && cmd.command.is_some() {
            anyhow::bail!(
                "Command '{}' in {} has both 'webview: true' and 'command' — webview opens a URL, not a command",
                cmd.key,
                context
            );
        }
        if cmd.navigate && !cmd.webview {
            anyhow::bail!(
                "Command '{}' in {} has 'navigate: true' without 'webview: true' — navigate only applies to the webview window",
                cmd.key,
                context
            );
        }
        if cmd.maximized && !cmd.webview {
            anyhow::bail!(
                "Command '{}' in {} has 'maximized: true' without 'webview: true' — maximized only applies to the webview window",
                cmd.key,
                context
            );
        }
    }
    validate_hotkeys(commands, context)
}

fn validate_commands(config: &Config) -> Result<()> {
    for_each_command_list(config, validate_command_list)
}

fn for_each_command_list(
    config: &Config,
    check: impl Fn(&[ProjectCommand], &str) -> Result<()>,
) -> Result<()> {
    if let Some(ref global) = config.global {
        check(global, "global commands")?;
    }
    for client in &config.clients {
        if let Some(ref commands) = client.commands {
            check(commands, &format!("client '{}'", client.name))?;
        }
        if let Some(ref projects) = client.projects {
            let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
            for project in projects {
                if !seen.insert(project.name.as_str()) {
                    anyhow::bail!(
                        "Duplicate project name '{}' in client '{}'",
                        project.name,
                        client.name
                    );
                }
                if let Some(ref commands) = project.commands {
                    check(
                        commands,
                        &format!("project '{}' in client '{}'", project.name, client.name),
                    )?;
                }
            }
        }
    }
    Ok(())
}

pub struct ConfigManager {
    config: Config,
    config_path: PathBuf,
    raw_yaml: Option<Value>,
    local_clients: Vec<Client>,
}

impl ConfigManager {
    pub fn new() -> Result<Self> {
        let config_path = dirs::home_dir()
            .context("Unable to determine home directory")?
            .join(".project-switch.yml");

        let (config, raw_yaml, local_clients) = Self::load_config(&config_path)?;

        Ok(Self {
            config,
            config_path,
            raw_yaml,
            local_clients,
        })
    }

    fn load_config(path: &PathBuf) -> Result<(Config, Option<Value>, Vec<Client>)> {
        if path.exists() {
            let contents = read_and_migrate(path)?;

            let local_config: Config =
                serde_yaml::from_str(&contents).context("Failed to parse config file")?;

            let raw_yaml: Value = serde_yaml::from_str(&contents)
                .context("Failed to parse config file as raw YAML")?;

            validate_commands(&local_config)?;

            let local_clients = local_config.clients.clone();

            // Handle include
            let config = if let Some(ref include_path) = local_config.include {
                let resolved = expand_tilde(include_path);
                if resolved.exists() {
                    let base_contents = read_and_migrate(&resolved)?;
                    let base_config: Config =
                        serde_yaml::from_str(&base_contents).with_context(|| {
                            format!("Failed to parse included config: {}", resolved.display())
                        })?;
                    validate_commands(&base_config).with_context(|| {
                        format!("Invalid included config: {}", resolved.display())
                    })?;
                    let merged = merge_configs(base_config, local_config);
                    for_each_command_list(&merged, validate_hotkeys)?;
                    merged
                } else {
                    eprintln!("Warning: included config not found: {}", resolved.display());
                    local_config
                }
            } else {
                local_config
            };

            Ok((config, Some(raw_yaml), local_clients))
        } else {
            let default_contents = "clients: []\n";
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
                // Update currentClient
                let current_client_key = Value::String("currentClient".to_string());
                if let Some(ref current) = self.config.current_client {
                    map.insert(current_client_key, Value::String(current.clone()));
                } else {
                    map.remove(&current_client_key);
                }

                // Update currentProject
                let current_project_key = Value::String("currentProject".to_string());
                if let Some(ref current) = self.config.current_project {
                    map.insert(current_project_key, Value::String(current.clone()));
                } else {
                    map.remove(&current_project_key);
                }

                // Update clients array (only local clients, not merged)
                let clients_key = Value::String("clients".to_string());
                let clients_value = serde_yaml::to_value(&self.local_clients)
                    .context("Failed to serialize clients")?;
                map.insert(clients_key, clients_value);
            }
            raw.clone()
        } else {
            // No existing file, serialize a config with only local clients
            let local_config = Config {
                include: self.config.include.clone(),
                current_client: self.config.current_client.clone(),
                current_project: self.config.current_project.clone(),
                default_browser: self.config.default_browser.clone(),
                global: self.config.global.clone(),
                shortcuts: self.config.shortcuts.clone(),
                webserver: self.config.webserver.clone(),
                webservers: self.config.webservers.clone(),
                monitor: self.config.monitor,
                paprika: self.config.paprika.clone(),
                calendar: self.config.calendar.clone(),
                clients: self.local_clients.clone(),
            };
            serde_yaml::to_value(&local_config).context("Failed to serialize config")?
        };

        self.write_yaml(&yaml_value)
    }

    fn write_yaml(&self, yaml_value: &Value) -> Result<()> {
        let yaml = serde_yaml::to_string(yaml_value).context("Failed to serialize config")?;

        fs::write(&self.config_path, yaml).with_context(|| {
            format!(
                "Failed to write config file: {}",
                self.config_path.display()
            )
        })
    }

    pub fn get_paprika_config(&self) -> Option<&PaprikaConfig> {
        self.config.paprika.as_ref()
    }

    fn edit_local_section(
        &mut self,
        key: &str,
        edit: impl FnOnce(&mut serde_yaml::Mapping),
    ) -> Result<()> {
        let raw = self
            .raw_yaml
            .get_or_insert_with(|| Value::Mapping(serde_yaml::Mapping::new()));
        let Value::Mapping(map) = raw else {
            anyhow::bail!("Config file is not a YAML mapping");
        };
        let section = map
            .entry(Value::String(key.to_string()))
            .or_insert(Value::Null);
        if !section.is_mapping() {
            *section = Value::Mapping(serde_yaml::Mapping::new());
        }
        if let Value::Mapping(section) = section {
            edit(section);
        }
        let raw = raw.clone();
        self.write_yaml(&raw)
    }

    pub fn set_local_paprika_token(&mut self, token: &str) -> Result<()> {
        self.edit_local_section("paprika", |section| {
            section.insert(
                Value::String("token".to_string()),
                Value::String(token.to_string()),
            );
        })?;

        self.config
            .paprika
            .get_or_insert_with(Default::default)
            .token = Some(token.to_string());
        Ok(())
    }

    pub fn get_calendar_config(&self) -> Option<&CalendarConfig> {
        self.config.calendar.as_ref()
    }

    pub fn set_local_calendar_token(&mut self, label: &str, token: &str) -> Result<()> {
        self.edit_local_section("calendar", |section| {
            let accounts = section
                .entry(Value::String("accounts".to_string()))
                .or_insert(Value::Null);
            if !accounts.is_sequence() {
                *accounts = Value::Sequence(Vec::new());
            }
            let Value::Sequence(accounts) = accounts else {
                return;
            };
            let token_key = Value::String("refreshToken".to_string());
            let token = Value::String(token.to_string());
            match accounts
                .iter_mut()
                .find(|a| a.get("label").and_then(Value::as_str) == Some(label))
            {
                Some(Value::Mapping(account)) => {
                    account.insert(token_key, token);
                }
                _ => {
                    let mut account = serde_yaml::Mapping::new();
                    account.insert(
                        Value::String("label".to_string()),
                        Value::String(label.to_string()),
                    );
                    account.insert(token_key, token);
                    accounts.push(Value::Mapping(account));
                }
            }
        })?;

        let accounts = &mut self
            .config
            .calendar
            .get_or_insert_with(Default::default)
            .accounts;
        let refresh_token = Some(token.to_string());
        match accounts.iter_mut().find(|a| a.label == label) {
            Some(account) => account.refresh_token = refresh_token,
            None => accounts.push(CalendarAccount {
                label: label.to_string(),
                refresh_token,
            }),
        }
        Ok(())
    }

    pub fn get_clients(&self) -> &Vec<Client> {
        &self.config.clients
    }

    pub fn get_current_client(&self) -> Option<&String> {
        self.config.current_client.as_ref()
    }

    pub fn get_current_project(&self) -> Option<&String> {
        self.config.current_project.as_ref()
    }

    /// Persist the active client and (optionally) nested project selection.
    /// Passing `None` for `project_name` clears the current project.
    pub fn set_current_selection(
        &mut self,
        client_name: &str,
        project_name: Option<&str>,
    ) -> Result<()> {
        if !self.client_exists(client_name) {
            anyhow::bail!("Client '{}' not found", client_name);
        }

        if let Some(project) = project_name {
            if !self.project_exists(client_name, project) {
                anyhow::bail!(
                    "Project '{}' not found in client '{}'",
                    project,
                    client_name
                );
            }
        }

        self.config.current_client = Some(client_name.to_string());
        self.config.current_project = project_name.map(|s| s.to_string());
        self.save_config()?;
        Ok(())
    }

    pub fn client_exists(&self, name: &str) -> bool {
        self.config.clients.iter().any(|c| c.name == name)
    }

    pub fn project_exists(&self, client_name: &str, project_name: &str) -> bool {
        self.get_client(client_name)
            .and_then(|c| c.projects.as_ref())
            .map(|projects| projects.iter().any(|p| p.name == project_name))
            .unwrap_or(false)
    }

    pub fn get_client(&self, name: &str) -> Option<&Client> {
        self.config.clients.iter().find(|c| c.name == name)
    }

    /// Returns the current client name and its configuration, or None if not set/found.
    pub fn resolve_current_client(&self) -> Option<(&String, &Client)> {
        let name = self.get_current_client()?;
        self.get_client(name).map(|c| (name, c))
    }

    /// Returns the resolved current selection: client plus optional nested project.
    /// The project is only returned if both keys are set and reference valid entries.
    pub fn resolve_current(&self) -> Option<ResolvedSelection<'_>> {
        let (client_name, client) = self.resolve_current_client()?;
        let project = self.get_current_project().and_then(|pname| {
            client
                .projects
                .as_ref()
                .and_then(|projects| projects.iter().find(|p| &p.name == pname))
                .map(|p| (pname, p))
        });
        Some((client_name, client, project))
    }

    /// Resolve a command by key using the active selection's effective command set.
    /// Precedence: project > client > global.
    pub fn get_effective_command(&self, command_key: &str) -> Option<&ProjectCommand> {
        let resolved = self.resolve_current()?;
        let (_, client, project) = resolved;

        if let Some((_, project)) = project {
            if let Some(cmd) = project
                .commands
                .as_ref()
                .and_then(|cmds| cmds.iter().find(|c| c.key == command_key))
            {
                return Some(cmd);
            }
        }

        if let Some(cmd) = client
            .commands
            .as_ref()
            .and_then(|cmds| cmds.iter().find(|c| c.key == command_key))
        {
            return Some(cmd);
        }

        self.config
            .global
            .as_ref()
            .and_then(|cmds| cmds.iter().find(|c| c.key == command_key))
    }

    pub fn get_default_browser(&self) -> &str {
        self.config.default_browser.as_deref().unwrap_or("firefox")
    }

    pub fn get_monitor(&self) -> Option<u32> {
        self.config.monitor
    }

    pub fn get_global_commands(&self) -> Option<&Vec<ProjectCommand>> {
        self.config.global.as_ref()
    }

    pub fn get_shortcuts_config(&self) -> ShortcutsConfig {
        self.config.shortcuts.clone().unwrap_or_default()
    }

    pub fn get_include_path(&self) -> Option<&str> {
        self.config.include.as_deref()
    }
}

#[cfg(test)]
fn write_temp_config(contents: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let tmp = std::env::temp_dir().join(format!(
        "ps-test-{}-{}.yml",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&tmp, contents).unwrap();
    tmp
}

#[cfg(test)]
pub(crate) fn make_manager(contents: &str) -> ConfigManager {
    let tmp = write_temp_config(contents);
    let (config, raw_yaml, local_clients) = ConfigManager::load_config(&tmp).unwrap();
    ConfigManager {
        config,
        config_path: tmp,
        raw_yaml,
        local_clients,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn old_schema_migrates_then_switch_persists_both_keys() {
        let initial = "\
currentProject: Build & Deploy
defaultBrowser: firefox
projects:
- name: nero
  commands:
  - key: git
    url: https://x
- name: EventsAir
  commands:
  - key: ci
    url: https://y
  projects:
  - name: Build & Deploy
    commands:
    - key: home
      url: https://home.example
  - name: Other
    commands:
    - key: home
      url: https://other.example
";
        let mut cm = make_manager(initial);

        // After migration, config stale currentClient points to a non-existent client
        assert_eq!(
            cm.get_current_client().cloned(),
            Some("Build & Deploy".to_string())
        );
        assert!(!cm.client_exists("Build & Deploy"));
        assert!(cm.client_exists("EventsAir"));
        assert!(cm.project_exists("EventsAir", "Build & Deploy"));

        cm.set_current_selection("EventsAir", Some("Build & Deploy"))
            .unwrap();

        let written = fs::read_to_string(&cm.config_path).unwrap();
        println!("--- written after switch ---\n{}\n---", written);
        assert!(
            written.contains("currentClient: EventsAir"),
            "got: {}",
            written
        );
        assert!(
            written.contains("currentProject: Build & Deploy"),
            "got: {}",
            written
        );
    }

    #[test]
    fn reloading_new_schema_preserves_current_project() {
        let initial = "\
currentClient: EventsAir
currentProject: Build & Deploy
clients:
- name: EventsAir
  projects:
  - name: Build & Deploy
    commands:
    - key: home
      url: https://home.example
";
        let cm = make_manager(initial);
        assert_eq!(
            cm.get_current_client().cloned(),
            Some("EventsAir".to_string())
        );
        assert_eq!(
            cm.get_current_project().cloned(),
            Some("Build & Deploy".to_string())
        );

        // File must be unchanged on disk (no spurious migration).
        let on_disk = fs::read_to_string(&cm.config_path).unwrap();
        assert!(on_disk.contains("currentClient: EventsAir"));
        assert!(on_disk.contains("currentProject: Build & Deploy"));
    }

    #[test]
    fn pinned_defaults_false_and_omitted_when_false() {
        let cmd = ProjectCommand {
            key: "git".to_string(),
            url: Some("https://x".to_string()),
            command: None,
            browser: None,
            args: None,
            webview: false,
            navigate: false,
            maximized: false,
            pinned: false,
            hotkey: None,
        };
        let yaml = serde_yaml::to_string(&cmd).unwrap();
        assert!(!yaml.contains("pinned"), "got: {}", yaml);

        // Absent in YAML deserializes to false.
        let parsed: ProjectCommand = serde_yaml::from_str("key: git\nurl: https://x\n").unwrap();
        assert!(!parsed.pinned);
    }

    #[test]
    fn pinned_true_round_trips() {
        let parsed: ProjectCommand =
            serde_yaml::from_str("key: git\nurl: https://x\npinned: true\n").unwrap();
        assert!(parsed.pinned);
        let yaml = serde_yaml::to_string(&parsed).unwrap();
        assert!(yaml.contains("pinned: true"), "got: {}", yaml);
    }

    #[test]
    fn webservers_list_round_trips() {
        let yaml = "\
webservers:
- name: pc-wsl
  enabled: true
  target: wsl
  distro: Ubuntu
  port: 3100
- name: pc-windows
  target: native
  command: assist sessions web --no-open --port 3101
  port: 3101
";
        let parsed: Config = serde_yaml::from_str(yaml).unwrap();
        let servers = parsed.webservers.as_ref().unwrap();
        assert_eq!(servers[0].name, "pc-wsl");
        assert!(servers[0].settings.enabled);
        assert_eq!(servers[1].target.as_deref(), Some("native"));
        assert_eq!(servers[1].settings.port, Some(3101));
        let out = serde_yaml::to_string(&parsed).unwrap();
        assert!(
            out.contains("command: assist sessions web --no-open --port 3101"),
            "got: {}",
            out
        );
    }

    #[test]
    fn switch_from_migrated_stale_current_to_client_with_project() {
        let initial = "\
currentClient: Build & Deploy
clients:
- name: EventsAir
  projects:
  - name: Build & Deploy
    commands:
    - key: home
      url: https://example.com
";
        let mut cm = make_manager(initial);
        cm.set_current_selection("EventsAir", Some("Build & Deploy"))
            .unwrap();

        let written = fs::read_to_string(&cm.config_path).unwrap();
        println!("--- written ---\n{}\n---", written);
        assert!(
            written.contains("currentClient: EventsAir"),
            "got: {}",
            written
        );
        assert!(
            written.contains("currentProject: Build & Deploy"),
            "got: {}",
            written
        );
    }

    fn load_error(contents: &str) -> String {
        let path = write_temp_config(contents);
        match ConfigManager::load_config(&path) {
            Ok(_) => panic!("expected config load to fail"),
            Err(e) => format!("{e:#}"),
        }
    }

    fn global_browser(cm: &ConfigManager) -> Option<CommandBrowser> {
        cm.get_global_commands().unwrap()[0].browser.clone()
    }

    fn option(label: &str, value: &str) -> BrowserOption {
        BrowserOption {
            label: label.to_string(),
            value: value.to_string(),
        }
    }

    const BROWSER_LIST: &str = "\
global:
- key: member-dev
  url: https://member.dev.example/
  browser:
  - label: integrator
    value: firefox -P integrator
  - label: integrator+coach
    value: firefox -P coach
";

    #[test]
    fn browser_string_parses_as_single() {
        let cm = make_manager("global:\n- key: a\n  url: https://x\n  browser: firefox -P work\n");
        let browser = global_browser(&cm).unwrap();
        assert_eq!(
            browser,
            CommandBrowser::Single("firefox -P work".to_string())
        );
        assert_eq!(browser.first_value(), "firefox -P work");
        assert!(browser.choices().is_none());
    }

    #[test]
    fn browser_list_parses_in_config_order() {
        let browser = global_browser(&make_manager(BROWSER_LIST)).unwrap();
        let expected = [
            option("integrator", "firefox -P integrator"),
            option("integrator+coach", "firefox -P coach"),
        ];
        assert_eq!(browser.choices(), Some(&expected[..]));
        assert_eq!(browser.first_value(), "firefox -P integrator");
    }

    #[test]
    fn single_entry_browser_list_offers_no_choice() {
        let cm = make_manager(
            "global:\n- key: a\n  url: https://x\n  browser:\n  - label: only\n    value: chrome\n",
        );
        let browser = global_browser(&cm).unwrap();
        assert!(browser.choices().is_none());
        assert_eq!(browser.first_value(), "chrome");
    }

    #[test]
    fn browser_list_round_trips() {
        let cmd: ProjectCommand = serde_yaml::from_str(
            "key: a\nurl: https://x\nbrowser:\n- label: one\n  value: firefox -P one\n",
        )
        .unwrap();
        let reparsed: ProjectCommand =
            serde_yaml::from_str(&serde_yaml::to_string(&cmd).unwrap()).unwrap();
        assert_eq!(reparsed.browser, cmd.browser);
    }

    #[test]
    fn empty_browser_list_is_rejected() {
        let err = load_error("global:\n- key: a\n  url: https://x\n  browser: []\n");
        assert!(err.contains("empty 'browser' list"), "got: {err}");
    }

    #[test]
    fn duplicate_browser_labels_are_rejected() {
        let err = load_error(
            "\
clients:
- name: c
  commands:
  - key: a
    url: https://x
    browser:
    - label: same
      value: firefox -P one
    - label: same
      value: firefox -P two
",
        );
        assert!(err.contains("duplicate browser label 'same'"), "got: {err}");
        assert!(err.contains("client 'c'"), "got: {err}");
    }

    #[test]
    fn browser_list_outside_commands_is_rejected() {
        for (contents, field) in [
            (
                "defaultBrowser:\n- label: a\n  value: firefox\n",
                "defaultBrowser",
            ),
            (
                "clients:\n- name: c\n  browser:\n  - label: a\n    value: firefox\n",
                "clients[0].browser",
            ),
            (
                "clients:\n- name: c\n  projects:\n  - name: p\n    browser:\n    - label: a\n      value: firefox\n",
                "clients[0].projects[0].browser",
            ),
        ] {
            let err = load_error(contents);
            assert!(err.contains(field), "got: {err}");
            assert!(err.contains("expected a string"), "got: {err}");
        }
    }

    #[test]
    fn included_browser_list_is_validated() {
        let base = write_temp_config("global:\n- key: a\n  url: https://x\n  browser: []\n");
        let err = load_error(&format!("include: {}\n", base.display()));
        assert!(err.contains("Invalid included config"), "got: {err}");
        assert!(err.contains("empty 'browser' list"), "got: {err}");
    }

    #[test]
    fn local_browser_replaces_included_browser() {
        let base = write_temp_config(BROWSER_LIST);
        let cm = make_manager(&format!(
            "include: {}\nglobal:\n- key: member-dev\n  browser: chrome\n",
            base.display()
        ));
        assert_eq!(
            global_browser(&cm),
            Some(CommandBrowser::Single("chrome".to_string()))
        );

        let base =
            write_temp_config("global:\n- key: member-dev\n  url: https://x\n  browser: chrome\n");
        let local = BROWSER_LIST.replace(
            "global:\n",
            &format!("include: {}\nglobal:\n", base.display()),
        );
        let browser = global_browser(&make_manager(&local)).unwrap();
        assert_eq!(browser.choices().map(<[_]>::len), Some(2));
    }

    #[test]
    fn hotkey_omitted_when_unset_and_round_trips_when_set() {
        let unset: ProjectCommand = serde_yaml::from_str("key: a\nurl: https://x\n").unwrap();
        assert!(!serde_yaml::to_string(&unset).unwrap().contains("hotkey"));

        let set: ProjectCommand =
            serde_yaml::from_str("key: a\nurl: https://x\nhotkey: alt+a\n").unwrap();
        assert!(serde_yaml::to_string(&set)
            .unwrap()
            .contains("hotkey: alt+a"));
        assert_eq!(set.key_combo(), Some("alt+a".parse().unwrap()));
    }

    #[test]
    fn invalid_hotkey_is_rejected_naming_the_command() {
        let err = load_error("global:\n- key: assist\n  url: https://x\n  hotkey: a\n");
        assert!(err.contains("Command 'assist'"), "got: {err}");
        assert!(err.contains("invalid hotkey 'a'"), "got: {err}");
    }

    #[test]
    fn duplicate_hotkey_in_one_list_is_rejected() {
        let err = load_error(
            "\
clients:
- name: c
  commands:
  - key: one
    url: https://x
    hotkey: alt+a
  - key: two
    url: https://y
    hotkey: ALT+A
",
        );
        assert!(err.contains("Command 'two'"), "got: {err}");
        assert!(err.contains("already bound to 'one'"), "got: {err}");
    }

    #[test]
    fn same_hotkey_at_different_levels_is_allowed() {
        let cm = make_manager(
            "\
global:
- key: g
  url: https://x
  hotkey: alt+a
clients:
- name: c
  commands:
  - key: c
    url: https://y
    hotkey: alt+a
",
        );
        assert_eq!(cm.get_clients().len(), 1);
    }

    #[test]
    fn hotkey_merges_through_include_by_key() {
        let base = write_temp_config("global:\n- key: assist\n  url: https://x\n  hotkey: alt+a\n");
        let cm = make_manager(&format!(
            "include: {}\nglobal:\n- key: assist\n  pinned: true\n",
            base.display()
        ));
        assert_eq!(
            cm.get_global_commands().unwrap()[0].hotkey.as_deref(),
            Some("alt+a")
        );
    }

    #[test]
    fn paprika_section_is_opt_in() {
        assert_eq!(make_manager("clients: []\n").get_paprika_config(), None);
        assert_eq!(
            make_manager("paprika:\n").get_paprika_config(),
            Some(&PaprikaConfig::default())
        );
        let cm = make_manager("paprika:\n  image: my/paprika\n  token: abc\n");
        assert_eq!(
            cm.get_paprika_config(),
            Some(&PaprikaConfig {
                image: Some("my/paprika".to_string()),
                token: Some("abc".to_string()),
            })
        );
    }

    #[test]
    fn unknown_paprika_field_is_rejected() {
        let err = load_error("paprika:\n  password: x\n");
        assert!(err.contains("unknown field `password`"), "got: {err}");
    }

    #[test]
    fn local_paprika_fields_win_over_included() {
        let base = write_temp_config("paprika:\n  image: shared/paprika\n  token: old\n");
        let cm = make_manager(&format!(
            "include: {}\npaprika:\n  token: new\n",
            base.display()
        ));
        assert_eq!(
            cm.get_paprika_config(),
            Some(&PaprikaConfig {
                image: Some("shared/paprika".to_string()),
                token: Some("new".to_string()),
            })
        );

        let cm = make_manager(&format!("include: {}\n", base.display()));
        assert_eq!(
            cm.get_paprika_config().and_then(|p| p.token.as_deref()),
            Some("old")
        );
    }

    #[test]
    fn hotkey_clash_introduced_by_include_is_rejected() {
        let base = write_temp_config("global:\n- key: one\n  url: https://x\n  hotkey: alt+a\n");
        let err = load_error(&format!(
            "include: {}\nglobal:\n- key: two\n  url: https://y\n  hotkey: alt+a\n",
            base.display()
        ));
        assert!(err.contains("already bound to 'one'"), "got: {err}");
    }

    #[test]
    fn paprika_token_is_saved_to_local_config_only() {
        let base_contents = "paprika:\n  image: shared/paprika\n";
        let base = write_temp_config(base_contents);
        let mut cm = make_manager(&format!(
            "include: {}\npaprika:\n  image: local/paprika\n",
            base.display()
        ));
        cm.set_local_paprika_token("tok").unwrap();

        assert_eq!(fs::read_to_string(&base).unwrap(), base_contents);
        let written: Config =
            serde_yaml::from_str(&fs::read_to_string(&cm.config_path).unwrap()).unwrap();
        assert_eq!(
            written.paprika,
            Some(PaprikaConfig {
                image: Some("local/paprika".to_string()),
                token: Some("tok".to_string()),
            })
        );
        assert_eq!(
            cm.get_paprika_config().and_then(|p| p.token.as_deref()),
            Some("tok")
        );
    }

    #[test]
    fn paprika_token_creates_section_when_absent() {
        let mut cm = make_manager("clients: []\n");
        cm.set_local_paprika_token("tok").unwrap();
        let written = fs::read_to_string(&cm.config_path).unwrap();
        assert!(written.contains("paprika:\n  token: tok"), "got: {written}");
    }

    fn written_calendar_accounts(cm: &ConfigManager) -> Vec<CalendarAccount> {
        let contents = fs::read_to_string(&cm.config_path).unwrap();
        serde_yaml::from_str::<Config>(&contents)
            .unwrap()
            .calendar
            .unwrap()
            .accounts
    }

    fn account(label: &str, token: Option<&str>) -> CalendarAccount {
        CalendarAccount {
            label: label.to_string(),
            refresh_token: token.map(str::to_string),
        }
    }

    #[test]
    fn calendar_section_parses() {
        assert_eq!(make_manager("clients: []\n").get_calendar_config(), None);
        assert_eq!(
            make_manager("calendar:\n").get_calendar_config(),
            Some(&CalendarConfig::default())
        );
        let cm = make_manager(
            "calendar:\n  clientId: id\n  clientSecret: secret\n  accounts:\n  - label: work\n    refreshToken: rt\n  - label: personal\n",
        );
        assert_eq!(
            cm.get_calendar_config(),
            Some(&CalendarConfig {
                client_id: Some("id".to_string()),
                client_secret: Some("secret".to_string()),
                accounts: vec![account("work", Some("rt")), account("personal", None)],
            })
        );
    }

    #[test]
    fn unknown_calendar_field_is_rejected() {
        let err = load_error("calendar:\n  password: x\n");
        assert!(err.contains("unknown field `password`"), "got: {err}");
        let err = load_error("calendar:\n  accounts:\n  - label: a\n    token: x\n");
        assert!(err.contains("unknown field `token`"), "got: {err}");
    }

    #[test]
    fn local_calendar_fields_merge_over_included() {
        let base = write_temp_config(
            "calendar:\n  clientId: shared\n  clientSecret: s\n  accounts:\n  - label: work\n",
        );
        let cm = make_manager(&format!(
            "include: {}\ncalendar:\n  accounts:\n  - label: work\n    refreshToken: rt\n  - label: personal\n",
            base.display()
        ));
        assert_eq!(
            cm.get_calendar_config(),
            Some(&CalendarConfig {
                client_id: Some("shared".to_string()),
                client_secret: Some("s".to_string()),
                accounts: vec![account("work", Some("rt")), account("personal", None)],
            })
        );
    }

    #[test]
    fn calendar_token_is_saved_to_local_config_only() {
        let base_contents = "calendar:\n  clientId: shared\n  accounts:\n  - label: work\n";
        let base = write_temp_config(base_contents);
        let mut cm = make_manager(&format!(
            "include: {}\ncalendar:\n  accounts:\n  - label: personal\n    refreshToken: old\n",
            base.display()
        ));
        cm.set_local_calendar_token("work", "new-work").unwrap();
        cm.set_local_calendar_token("personal", "new-personal")
            .unwrap();

        assert_eq!(fs::read_to_string(&base).unwrap(), base_contents);
        assert_eq!(
            written_calendar_accounts(&cm),
            vec![
                account("personal", Some("new-personal")),
                account("work", Some("new-work")),
            ]
        );
        assert_eq!(
            cm.get_calendar_config().unwrap().accounts,
            vec![
                account("work", Some("new-work")),
                account("personal", Some("new-personal")),
            ]
        );
    }

    #[test]
    fn calendar_token_creates_section_when_absent() {
        let mut cm = make_manager("clients: []\n");
        cm.set_local_calendar_token("work", "rt").unwrap();
        assert_eq!(
            written_calendar_accounts(&cm),
            vec![account("work", Some("rt"))]
        );
    }

    #[test]
    fn command_hotkey_clashing_with_calendar_is_rejected() {
        let err = load_error("global:\n- key: cal\n  url: https://x\n  hotkey: alt+c\n");
        assert!(err.contains("Command 'cal'"), "got: {err}");
        assert!(
            err.contains("already bound to 'the calendar sidebar'"),
            "got: {err}"
        );
    }
}
