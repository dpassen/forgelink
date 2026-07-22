use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, anyhow};
use directories::BaseDirs;
use forgelink::{Forge, ForgeTarget};
use serde::Deserialize;

#[derive(Debug, Default)]
pub struct Config {
    hosts: HashMap<String, ForgeTarget>,
}

impl Config {
    pub fn load(explicit: Option<&Path>) -> anyhow::Result<Self> {
        let required = explicit.is_some();
        let xdg = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from);
        let native = BaseDirs::new().map(|dirs| dirs.config_dir().to_path_buf());
        let Some(path) = select_path(explicit, xdg.as_deref(), native.as_deref()) else {
            return Ok(Self::default());
        };

        Self::load_path(&path, required)
    }

    fn load_path(path: &Path, required: bool) -> anyhow::Result<Self> {
        let contents = match fs::read_to_string(path) {
            Ok(contents) => contents,
            Err(error) if !required && error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to read config file {}", path.display()));
            }
        };

        Self::parse(&contents)
            .with_context(|| format!("failed to parse config file {}", path.display()))
    }

    fn parse(contents: &str) -> anyhow::Result<Self> {
        let file: ConfigFile = toml::from_str(contents)?;
        let mut hosts = HashMap::with_capacity(file.hosts.len());

        for entry in file.hosts {
            let trimmed = entry.host.trim();
            if trimmed.is_empty() {
                return Err(anyhow!("host must not be empty"));
            }
            if trimmed != entry.host {
                return Err(anyhow!(
                    "host '{}' must not have leading or trailing whitespace",
                    entry.host
                ));
            }

            let key = entry.host.to_ascii_lowercase();
            if hosts.contains_key(&key) {
                return Err(anyhow!("duplicate host '{}'", entry.host));
            }

            let forge = parse_forge(&entry.forge)
                .with_context(|| format!("invalid target for host '{}'", entry.host))?;
            let target = ForgeTarget::new(&entry.base_url, forge)
                .with_context(|| format!("invalid target for host '{}'", entry.host))?;
            hosts.insert(key, target);
        }

        Ok(Self { hosts })
    }

    pub fn target_for(&self, host: &str) -> Option<ForgeTarget> {
        self.hosts.get(&host.to_ascii_lowercase()).cloned()
    }
}

fn select_path(
    explicit: Option<&Path>,
    xdg: Option<&Path>,
    native: Option<&Path>,
) -> Option<PathBuf> {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| {
            xdg.filter(|path| path.is_absolute())
                .map(|path| path.join("forgelink/config.toml"))
        })
        .or_else(|| native.map(|path| path.join("forgelink/config.toml")))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigFile {
    #[serde(default)]
    hosts: Vec<HostEntry>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
struct HostEntry {
    host: String,
    base_url: String,
    forge: String,
}

fn parse_forge(value: &str) -> anyhow::Result<Forge> {
    match value {
        "github" => Ok(Forge::GitHub),
        "gitlab" => Ok(Forge::GitLab),
        "sourcehut" => Ok(Forge::SourceHut),
        "bitbucket" => Ok(Forge::Bitbucket),
        "codeberg" => Ok(Forge::Codeberg),
        _ => Err(anyhow!("unsupported forge '{value}'")),
    }
}

#[cfg(test)]
mod tests;
