#![cfg(feature = "integration")]

mod common;

/// Integration tests for AuthTokenManager.
/// Translated from libmodal/modal-go/test/auth_token_manager_test.go

use base64::Engine;
use modal::auth_token_manager::parse_jwt_expiration;

fn mock_jwt(payload: &serde_json::Value) -> String {
    let header = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(r#"{"alg":"HS256","typ":"JWT"}"#);
    let payload_b64 = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .encode(serde_json::to_vec(payload).unwrap());
    format!("{}.{}.fake-signature", header, payload_b64)
}

#[test]
fn test_auth_token_manager_jwt_decode() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let exp = now + 3600;
    let jwt = mock_jwt(&serde_json::json!({"exp": exp}));

    let result = parse_jwt_expiration(&jwt).unwrap();
    assert_eq!(result, Some(exp));
}

#[test]
fn test_auth_token_manager_jwt_decode_no_exp() {
    let jwt = mock_jwt(&serde_json::json!({}));
    let result = parse_jwt_expiration(&jwt).unwrap();
    assert_eq!(result, None);
}

#[test]
fn test_auth_token_manager_jwt_decode_float_exp() {
    let jwt = mock_jwt(&serde_json::json!({"exp": 1700000000.5}));
    let result = parse_jwt_expiration(&jwt).unwrap();
    assert_eq!(result, Some(1700000000));
}

#[test]
fn test_auth_token_manager_malformed_jwt() {
    assert!(parse_jwt_expiration("only.two").is_err());
    assert!(parse_jwt_expiration("a.!!!.c").is_err());
    assert!(parse_jwt_expiration("").is_err());
}

#[test]
fn test_auth_token_manager_expired_token() {
    let exp = 1000;
    let jwt = mock_jwt(&serde_json::json!({"exp": exp}));
    let result = parse_jwt_expiration(&jwt).unwrap();
    assert_eq!(result, Some(1000));
}

#[test]
fn test_auth_token_manager_non_numeric_exp() {
    let jwt = mock_jwt(&serde_json::json!({"exp": "not-a-number"}));
    assert!(parse_jwt_expiration(&jwt).is_err());
}
