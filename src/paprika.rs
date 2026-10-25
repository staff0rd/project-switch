use crate::config::PaprikaConfig;
use anyhow::{Context, Result};
use clap::Subcommand;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub const DEFAULT_IMAGE: &str = "ghcr.io/staff0rd/paprika";
pub const AUTH_HINT: &str =
    "Paprika needs a login: run `project-switch grocery auth` in a terminal";
const CONTAINER_TOKEN_DIR: &str = "/home/paprika/.paprika";
const TOKEN_FILE: &str = "token";
const PROMPT_WITHOUT_TTY_ERROR: &str = "credentials not provided";
const DEFAULT_LIST_MARKER: &str = " (default)";

#[derive(Debug, Clone, PartialEq, Subcommand)]
pub enum GroceryCommand {
    #[command(about = "Log in to Paprika and save the token to ~/.project-switch.yml")]
    Auth,
    #[command(about = "Show unpurchased items on every grocery list")]
    List,
    #[command(about = "Add an item to the default grocery list")]
    Add {
        item: String,
        #[arg(long, help = "Grocery list name (case-insensitive)")]
        list: Option<String>,
    },
    #[command(about = "Remove an item from the default grocery list")]
    Remove {
        item: String,
        #[arg(long, help = "Grocery list name (case-insensitive)")]
        list: Option<String>,
    },
}

impl GroceryCommand {
    fn container_args(&self) -> Vec<String> {
        let (name, item, list) = match self {
            GroceryCommand::Auth => ("auth", None, None),
            GroceryCommand::List => ("list", None, None),
            GroceryCommand::Add { item, list } => ("add", Some(item), list.as_ref()),
            GroceryCommand::Remove { item, list } => ("remove", Some(item), list.as_ref()),
        };
        let mut args = vec![name.to_string()];
        args.extend(item.cloned());
        if let Some(list) = list {
            args.extend(["--list".to_string(), list.clone()]);
        }
        args
    }
}

fn image(config: &PaprikaConfig) -> &str {
    config.image.as_deref().unwrap_or(DEFAULT_IMAGE)
}

fn docker_args(
    image: &str,
    token_dir: &Path,
    user: Option<&str>,
    command: &GroceryCommand,
) -> Vec<String> {
    let mut args = vec!["run".to_string(), "--rm".to_string()];
    if *command == GroceryCommand::Auth {
        args.push("-it".to_string());
    }
    if let Some(user) = user {
        args.extend([
            "--user".to_string(),
            user.to_string(),
            "-e".to_string(),
            "HOME=/home/paprika".to_string(),
        ]);
    }
    args.push("--mount".to_string());
    args.push(format!(
        "type=bind,src={},dst={CONTAINER_TOKEN_DIR}",
        token_dir.display()
    ));
    args.push(image.to_string());
    args.extend(command.container_args());
    args
}

struct TokenDir {
    path: PathBuf,
}

impl TokenDir {
    fn create() -> Result<Self> {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or_default();
        let path = std::env::temp_dir().join(format!(
            "project-switch-paprika-{}-{nanos}",
            std::process::id()
        ));
        fs::create_dir(&path)
            .with_context(|| format!("Failed to create temp dir: {}", path.display()))?;
        Ok(Self { path })
    }

    fn token_path(&self) -> PathBuf {
        self.path.join(TOKEN_FILE)
    }

    #[cfg(unix)]
    fn owner(&self) -> Option<String> {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::metadata(&self.path).ok()?;
        Some(format!("{}:{}", meta.uid(), meta.gid()))
    }

    #[cfg(not(unix))]
    fn owner(&self) -> Option<String> {
        None
    }

    fn docker_command(&self, config: &PaprikaConfig, command: &GroceryCommand) -> Command {
        let mut docker = Command::new("docker");
        docker.args(docker_args(
            image(config),
            &self.path,
            self.owner().as_deref(),
            command,
        ));
        docker
    }
}

impl Drop for TokenDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn docker_spawn_error(e: std::io::Error) -> anyhow::Error {
    if e.kind() == std::io::ErrorKind::NotFound {
        anyhow::anyhow!("Docker not found: install Docker and make sure `docker` is on PATH")
    } else {
        anyhow::anyhow!("Failed to run docker: {e}")
    }
}

fn configured_token(config: &PaprikaConfig) -> Result<&str> {
    config
        .token
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .context(AUTH_HINT)
}

fn interpret_output(success: bool, stdout: &str, stderr: &str) -> Result<String> {
    if success {
        return Ok(stdout.to_string());
    }
    if stderr.contains(PROMPT_WITHOUT_TTY_ERROR) {
        anyhow::bail!(AUTH_HINT);
    }
    let stderr = stderr.trim();
    if stderr.is_empty() {
        anyhow::bail!("Paprika container failed with no error output");
    }
    anyhow::bail!("{stderr}")
}

pub fn run(config: &PaprikaConfig, command: &GroceryCommand) -> Result<String> {
    let token = configured_token(config)?;
    let dir = TokenDir::create()?;
    fs::write(dir.token_path(), token).context("Failed to write Paprika token to temp dir")?;
    let output = dir
        .docker_command(config, command)
        .stdin(Stdio::null())
        .output()
        .map_err(docker_spawn_error)?;
    interpret_output(
        output.status.success(),
        &String::from_utf8_lossy(&output.stdout),
        &String::from_utf8_lossy(&output.stderr),
    )
}

fn parse_default_list(output: &str) -> Vec<String> {
    output
        .lines()
        .skip_while(|line| !line.ends_with(DEFAULT_LIST_MARKER) || line.starts_with(' '))
        .skip(1)
        .take_while(|line| line.starts_with(' '))
        .filter_map(|line| line.trim_start().strip_prefix("- "))
        .map(str::to_string)
        .collect()
}

pub fn default_list_items(config: &PaprikaConfig) -> Result<Vec<String>> {
    Ok(parse_default_list(&run(config, &GroceryCommand::List)?))
}

pub fn authenticate(config: &PaprikaConfig) -> Result<String> {
    let dir = TokenDir::create()?;
    let status = dir
        .docker_command(config, &GroceryCommand::Auth)
        .status()
        .map_err(docker_spawn_error)?;
    if !status.success() {
        anyhow::bail!("Paprika login failed ({status})");
    }
    let token = fs::read_to_string(dir.token_path())
        .context("Paprika login finished without saving a token")?;
    let token = token.trim();
    if token.is_empty() {
        anyhow::bail!("Paprika login saved an empty token");
    }
    Ok(token.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn add(item: &str, list: Option<&str>) -> GroceryCommand {
        GroceryCommand::Add {
            item: item.to_string(),
            list: list.map(str::to_string),
        }
    }

    fn args(command: &GroceryCommand, user: Option<&str>) -> Vec<String> {
        docker_args(DEFAULT_IMAGE, Path::new("/tmp/tok"), user, command)
    }

    const MOUNT: &str = "type=bind,src=/tmp/tok,dst=/home/paprika/.paprika";

    #[test]
    fn auth_runs_interactively() {
        assert_eq!(
            args(&GroceryCommand::Auth, None),
            [
                "run",
                "--rm",
                "-it",
                "--mount",
                MOUNT,
                DEFAULT_IMAGE,
                "auth"
            ]
        );
    }

    #[test]
    fn non_auth_commands_are_not_interactive() {
        assert_eq!(
            args(&GroceryCommand::List, None),
            ["run", "--rm", "--mount", MOUNT, DEFAULT_IMAGE, "list"]
        );
        assert_eq!(
            args(&add("Milk", None), None)[3..],
            [MOUNT, DEFAULT_IMAGE, "add", "Milk"]
        );
    }

    #[test]
    fn list_flag_is_passed_through() {
        let remove = GroceryCommand::Remove {
            item: "Nails".to_string(),
            list: Some("Hardware".to_string()),
        };
        assert_eq!(
            args(&remove, None)[4..],
            [DEFAULT_IMAGE, "remove", "Nails", "--list", "Hardware"]
        );
        assert_eq!(
            args(&add("Oat milk", Some("Groceries")), None)[5..],
            ["add", "Oat milk", "--list", "Groceries"]
        );
    }

    #[test]
    fn host_user_runs_container_with_mounted_home() {
        assert_eq!(
            args(&GroceryCommand::List, Some("1000:1000"))[2..6],
            ["--user", "1000:1000", "-e", "HOME=/home/paprika"]
        );
    }

    #[test]
    fn image_override_is_used() {
        let config = PaprikaConfig {
            image: Some("paprika".to_string()),
            token: None,
        };
        assert_eq!(image(&config), "paprika");
        assert_eq!(image(&PaprikaConfig::default()), DEFAULT_IMAGE);
    }

    #[test]
    fn missing_token_asks_for_auth() {
        for token in [None, Some(""), Some("  ")] {
            let config = PaprikaConfig {
                image: None,
                token: token.map(str::to_string),
            };
            let err = run(&config, &GroceryCommand::List).unwrap_err();
            assert_eq!(err.to_string(), AUTH_HINT);
        }
    }

    #[test]
    fn rejected_token_asks_for_auth() {
        let stderr = "Saved token was rejected, please log in again.\n\nError: credentials not provided (run the container with -it)\n";
        let err = interpret_output(false, "", stderr).unwrap_err();
        assert_eq!(err.to_string(), AUTH_HINT);
    }

    #[test]
    fn container_errors_surface_stderr() {
        let err = interpret_output(false, "", "Error: unknown list \"Nope\"\n").unwrap_err();
        assert_eq!(err.to_string(), "Error: unknown list \"Nope\"");
        assert!(interpret_output(false, "", "").is_err());
    }

    #[test]
    fn success_returns_stdout() {
        assert_eq!(
            interpret_output(true, "Added \"Milk\" to Groceries.\n", "").unwrap(),
            "Added \"Milk\" to Groceries.\n"
        );
    }

    #[test]
    fn default_list_items_are_parsed_from_list_output() {
        let output = "Hardware\n  - Nails\n\nGroceries (default)\n  - Milk\n  - Oat milk\n\nParty\n  - Cake\n\n";
        assert_eq!(parse_default_list(output), ["Milk", "Oat milk"]);
    }

    #[test]
    fn empty_default_list_has_no_items() {
        let output = "Groceries (default)\n  (no items)\n\nHardware\n  - Nails\n\n";
        assert!(parse_default_list(output).is_empty());
        assert!(parse_default_list("").is_empty());
    }

    #[test]
    fn token_dir_is_removed_on_drop() {
        let dir = TokenDir::create().unwrap();
        fs::write(dir.token_path(), "tok").unwrap();
        let path = dir.path.clone();
        drop(dir);
        assert!(!path.exists());
    }
}
