/// gRPC interceptor utilities for the Modal client.

use crate::config::Profile;

/// Inject required Modal headers into gRPC metadata.
pub fn inject_required_headers(
    profile: &Profile,
    sdk_version: &str,
) -> Result<Vec<(String, String)>, crate::error::ModalError> {
    if profile.token_id.is_empty() || profile.token_secret.is_empty() {
        return Err(crate::error::ModalError::Config(
            "missing token_id or token_secret, please set in .modal.toml, environment variables, or via ClientParams".to_string(),
        ));
    }

    Ok(vec![
        ("x-modal-client-type".to_string(), "9".to_string()), // CLIENT_TYPE_LIBMODAL_GO
        ("x-modal-client-version".to_string(), "1.0.0".to_string()),
        ("x-modal-libmodal-version".to_string(), format!("modal-rs/{}", sdk_version)),
        ("x-modal-token-id".to_string(), profile.token_id.clone()),
        ("x-modal-token-secret".to_string(), profile.token_secret.clone()),
    ])
}
