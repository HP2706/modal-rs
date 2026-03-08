use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use crate::error::ModalError;

/// Profile holds a fully-resolved configuration ready for use by the client.
#[derive(Debug, Clone, Default)]
pub struct Profile {
    pub server_url: String,
    pub token_id: String,
    pub token_secret: String,
    pub environment: String,
    pub image_builder_version: String,
    pub log_level: String,
}

impl Profile {
    pub fn is_localhost(&self) -> bool {
        match url::Url::parse(&self.server_url) {
            Ok(parsed) => match parsed.host() {
                Some(url::Host::Domain(d)) => d == "localhost",
                Some(url::Host::Ipv4(ip)) => {
                    ip == std::net::Ipv4Addr::new(127, 0, 0, 1)
                        || ip == std::net::Ipv4Addr::new(172, 21, 0, 1)
                }
                Some(url::Host::Ipv6(ip)) => ip == std::net::Ipv6Addr::LOCALHOST,
                None => false,
            },
            Err(_) => false,
        }
    }
}

/// rawProfile mirrors the TOML structure on disk.
#[derive(Debug, Deserialize, Default, Clone)]
pub(crate) struct RawProfile {
    #[serde(default)]
    server_url: String,
    #[serde(default)]
    token_id: String,
    #[serde(default)]
    token_secret: String,
    #[serde(default)]
    environment: String,
    #[serde(default)]
    image_builder_version: String,
    #[serde(default)]
    loglevel: String,
    #[serde(default)]
    active: bool,
}

pub(crate) type Config = HashMap<String, RawProfile>;

pub(crate) fn config_file_path() -> Result<PathBuf, ModalError> {
    if let Ok(config_path) = env::var("MODAL_CONFIG_PATH") {
        if !config_path.is_empty() {
            return Ok(PathBuf::from(config_path));
        }
    }

    let home = dirs::home_dir()
        .ok_or_else(|| ModalError::Config("cannot locate homedir".to_string()))?;
    Ok(home.join(".modal.toml"))
}

/// readConfigFile loads the Modal config file, returning an empty config if the file
/// does not exist.
pub(crate) fn read_config_file() -> Result<Config, ModalError> {
    let path = config_file_path()?;

    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let cfg: Config = toml::from_str(&content)
                .map_err(|e| ModalError::Config(format!("parsing {}: {}", path.display(), e)))?;
            Ok(cfg)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Config::new()),
        Err(e) => Err(ModalError::Config(format!(
            "reading {}: {}",
            path.display(),
            e
        ))),
    }
}

pub(crate) fn get_profile(name: &str, cfg: &Config) -> Profile {
    let mut profile_name = name.to_string();

    if profile_name.is_empty() {
        for (n, p) in cfg {
            if p.active {
                profile_name = n.clone();
                break;
            }
        }
    }

    let raw = if !profile_name.is_empty() {
        cfg.get(&profile_name).cloned().unwrap_or_default()
    } else {
        RawProfile::default()
    };

    let server_url = first_non_empty(&[
        &env::var("MODAL_SERVER_URL").unwrap_or_default(),
        &raw.server_url,
        "https://api.modal.com:443",
    ]);
    let token_id = first_non_empty(&[
        &env::var("MODAL_TOKEN_ID").unwrap_or_default(),
        &raw.token_id,
    ]);
    let token_secret = first_non_empty(&[
        &env::var("MODAL_TOKEN_SECRET").unwrap_or_default(),
        &raw.token_secret,
    ]);
    let environment = first_non_empty(&[
        &env::var("MODAL_ENVIRONMENT").unwrap_or_default(),
        &raw.environment,
    ]);
    let image_builder_version = first_non_empty(&[
        &env::var("MODAL_IMAGE_BUILDER_VERSION").unwrap_or_default(),
        &raw.image_builder_version,
    ]);
    let log_level = first_non_empty(&[
        &env::var("MODAL_LOGLEVEL").unwrap_or_default(),
        &raw.loglevel,
    ]);

    Profile {
        server_url,
        token_id,
        token_secret,
        environment,
        image_builder_version,
        log_level,
    }
}

fn first_non_empty(values: &[&str]) -> String {
    for v in values {
        if !v.is_empty() {
            return v.to_string();
        }
    }
    String::new()
}

pub fn environment_name(environment: &str, profile: &Profile) -> String {
    first_non_empty(&[environment, &profile.environment])
}

pub fn image_builder_version(version: &str, profile: &Profile) -> String {
    first_non_empty(&[version, &profile.image_builder_version, "2024.10"])
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests modify env vars so they must run sequentially.
    // Combined into one test to avoid race conditions in parallel test execution.
    #[test]
    fn test_get_config_path() {
        // With env var
        unsafe { env::set_var("MODAL_CONFIG_PATH", "/custom/path/to/config.toml") };
        let path = config_file_path().unwrap();
        assert_eq!(path, PathBuf::from("/custom/path/to/config.toml"));

        // Without env var (empty)
        unsafe { env::set_var("MODAL_CONFIG_PATH", "") };
        let path = config_file_path().unwrap();
        let home = dirs::home_dir().unwrap();
        assert_eq!(path, home.join(".modal.toml"));

        unsafe { env::remove_var("MODAL_CONFIG_PATH") };
    }

    #[test]
    fn test_profile_is_localhost() {
        let p = Profile {
            server_url: "http://localhost:8889".to_string(),
            ..Default::default()
        };
        assert!(p.is_localhost());
    }

    #[test]
    fn test_profile_is_localhost_127() {
        let p = Profile {
            server_url: "http://127.0.0.1:8889".to_string(),
            ..Default::default()
        };
        assert!(p.is_localhost());
    }

    #[test]
    fn test_profile_is_localhost_ipv6() {
        let p = Profile {
            server_url: "http://[::1]:8889".to_string(),
            ..Default::default()
        };
        assert!(p.is_localhost());
    }

    #[test]
    fn test_profile_is_localhost_172() {
        let p = Profile {
            server_url: "http://172.21.0.1:8889".to_string(),
            ..Default::default()
        };
        assert!(p.is_localhost());
    }

    #[test]
    fn test_profile_is_not_localhost() {
        let p = Profile {
            server_url: "https://api.modal.com:443".to_string(),
            ..Default::default()
        };
        assert!(!p.is_localhost());
    }
}
