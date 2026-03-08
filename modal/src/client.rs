use crate::config::Profile;

/// Client exposes services for interacting with Modal resources.
#[derive(Debug)]
pub struct Client {
    pub profile: Profile,
    pub sdk_version: String,
}

impl Client {
    /// Create a new client with default configuration.
    pub fn new() -> Result<Self, crate::error::ModalError> {
        Self::with_options(None)
    }

    /// Create a new client with custom options.
    pub fn with_options(params: Option<&ClientParams>) -> Result<Self, crate::error::ModalError> {
        let cfg = crate::config::read_config_file()?;
        let mut profile =
            crate::config::get_profile(&std::env::var("MODAL_PROFILE").unwrap_or_default(), &cfg);

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

        Ok(Client {
            profile,
            sdk_version: "0.1.0".to_string(),
        })
    }

    /// Returns the SDK version.
    pub fn version(&self) -> &str {
        &self.sdk_version
    }
}

/// ClientParams defines credentials and options for initializing the Modal client.
#[derive(Debug, Clone, Default)]
pub struct ClientParams {
    pub token_id: String,
    pub token_secret: String,
    pub environment: String,
}
