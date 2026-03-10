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

impl Profile {
    /// Resolve a Profile from environment variables and the config file (~/.modal.toml).
    /// Matches Go SDK's config resolution in `NewClient()`.
    ///
    /// Resolution order for each field:
    /// 1. Environment variables (MODAL_TOKEN_ID, MODAL_TOKEN_SECRET, etc.)
    /// 2. Active profile from config file
    /// 3. Built-in defaults
    pub fn from_config() -> Result<Self, ModalError> {
        Self::from_config_with_overrides(None)
    }

    /// Resolve a Profile from environment variables and the config file, then apply
    /// any overrides from the given `ClientParams`.
    /// Matches Go SDK's `NewClientWithOptions()`.
    pub fn from_config_with_overrides(params: Option<&ClientParams>) -> Result<Self, ModalError> {
        let cfg = read_config_file()?;
        let profile_name = env::var("MODAL_PROFILE").unwrap_or_default();
        let mut profile = get_profile(&profile_name, &cfg);

        if let Some(p) = params {
            if !p.token_id.is_empty() {
                profile.token_id = p.token_id.clone();
            }
            if !p.token_secret.is_empty() {
                profile.token_secret = p.token_secret.clone();
            }
            if !p.environment.is_empty() {
                profile.environment = p.environment.clone();
            }
        }

        Ok(profile)
    }
}

/// ClientParams defines credentials and options for initializing the Modal client.
/// Matches Go SDK's `ClientParams` struct.
#[derive(Debug, Clone, Default)]
pub struct ClientParams {
    pub token_id: String,
    pub token_secret: String,
    pub environment: String,
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

    // All Profile::from_config tests are combined into one test to avoid
    // env var race conditions when tests run in parallel.
    #[test]
    fn test_profile_from_config() {
        // --- Part 1: env var resolution ---
        unsafe {
            env::set_var("MODAL_CONFIG_PATH", "/tmp/nonexistent_modal_test.toml");
            env::set_var("MODAL_TOKEN_ID", "ak-test123");
            env::set_var("MODAL_TOKEN_SECRET", "as-secret456");
            env::set_var("MODAL_ENVIRONMENT", "test-env");
            env::remove_var("MODAL_PROFILE");
        }

        let profile = Profile::from_config().unwrap();
        assert_eq!(profile.token_id, "ak-test123");
        assert_eq!(profile.token_secret, "as-secret456");
        assert_eq!(profile.environment, "test-env");
        assert_eq!(profile.server_url, "https://api.modal.com:443");

        // --- Part 2: overrides replace env values ---
        let params = ClientParams {
            token_id: "ak-override".to_string(),
            token_secret: "as-override".to_string(),
            environment: "override-env".to_string(),
        };
        let profile = Profile::from_config_with_overrides(Some(&params)).unwrap();
        assert_eq!(profile.token_id, "ak-override");
        assert_eq!(profile.token_secret, "as-override");
        assert_eq!(profile.environment, "override-env");

        // --- Part 3: empty overrides keep env values ---
        let params = ClientParams::default();
        let profile = Profile::from_config_with_overrides(Some(&params)).unwrap();
        assert_eq!(profile.token_id, "ak-test123");
        assert_eq!(profile.token_secret, "as-secret456");

        // --- Part 4: TOML file reading ---
        let tmp = std::env::temp_dir().join("modal_test_config.toml");
        std::fs::write(
            &tmp,
            r#"
[myprofile]
token_id = "ak-from-toml"
token_secret = "as-from-toml"
server_url = "https://custom.modal.com:443"
active = true
"#,
        )
        .unwrap();

        unsafe {
            env::set_var("MODAL_CONFIG_PATH", tmp.to_str().unwrap());
            env::remove_var("MODAL_TOKEN_ID");
            env::remove_var("MODAL_TOKEN_SECRET");
            env::remove_var("MODAL_SERVER_URL");
            env::remove_var("MODAL_PROFILE");
            env::remove_var("MODAL_ENVIRONMENT");
        }

        let profile = Profile::from_config().unwrap();
        assert_eq!(profile.token_id, "ak-from-toml");
        assert_eq!(profile.token_secret, "as-from-toml");
        assert_eq!(profile.server_url, "https://custom.modal.com:443");

        unsafe {
            env::remove_var("MODAL_CONFIG_PATH");
        }
        let _ = std::fs::remove_file(&tmp);

        // --- Part 5: ClientParams default ---
        let params = ClientParams::default();
        assert!(params.token_id.is_empty());
        assert!(params.token_secret.is_empty());
        assert!(params.environment.is_empty());
    }
}
