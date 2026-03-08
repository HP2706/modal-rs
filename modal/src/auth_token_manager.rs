use base64::Engine;

use crate::error::ModalError;

/// Parse JWT expiration from a JWT token string.
/// Returns Ok(Some(exp)) if the token has an exp claim.
/// Returns Ok(None) if the token has no exp claim.
/// Returns Err if the token is malformed.
pub fn parse_jwt_expiration(jwt: &str) -> Result<Option<i64>, ModalError> {
    let parts: Vec<&str> = jwt.split('.').collect();
    if parts.len() != 3 {
        return Err(ModalError::Other(format!(
            "malformed JWT: expected 3 parts, got {}",
            parts.len()
        )));
    }

    let payload_b64 = parts[1];

    let payload_json = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload_b64)
        .map_err(|e| ModalError::Other(format!("malformed JWT: base64 decode: {}", e)))?;

    let payload: serde_json::Value = serde_json::from_slice(&payload_json)
        .map_err(|e| ModalError::Other(format!("malformed JWT: json unmarshal: {}", e)))?;

    match payload.get("exp") {
        None => Ok(None),
        Some(exp_val) => {
            let exp = exp_val
                .as_i64()
                .or_else(|| exp_val.as_f64().map(|f| f as i64))
                .ok_or_else(|| {
                    ModalError::Other(format!(
                        "malformed JWT: exp not an integer: {}",
                        exp_val
                    ))
                })?;
            Ok(Some(exp))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn mock_jwt(exp: Option<serde_json::Value>) -> String {
        let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .encode(r#"{"alg":"HS256","typ":"JWT"}"#);

        let payload = match exp {
            Some(val) => {
                let map = serde_json::json!({"exp": val});
                serde_json::to_vec(&map).unwrap()
            }
            None => {
                let map = serde_json::json!({});
                serde_json::to_vec(&map).unwrap()
            }
        };

        let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&payload);
        format!("{}.{}.fake-signature", header, payload_b64)
    }

    #[test]
    fn test_parse_jwt_expiration_valid() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let exp = now + 3600;
        let jwt = mock_jwt(Some(serde_json::json!(exp)));

        let result = parse_jwt_expiration(&jwt).unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap(), exp);
    }

    #[test]
    fn test_parse_jwt_expiration_without_exp() {
        let jwt = mock_jwt(None);
        let result = parse_jwt_expiration(&jwt).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_jwt_expiration_malformed() {
        let jwt = "only.two";
        let result = parse_jwt_expiration(jwt);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_jwt_expiration_invalid_base64() {
        let jwt = "invalid.!!!invalid!!!.signature";
        let result = parse_jwt_expiration(jwt);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_jwt_expiration_non_numeric_exp() {
        let jwt = mock_jwt(Some(serde_json::json!("not-a-number")));
        let result = parse_jwt_expiration(&jwt);
        assert!(result.is_err());
    }
}
