#![cfg(feature = "integration")]

/// Integration tests for Modal Config module.
/// Translated from libmodal/modal-go/config_test.go

use modal::config::Profile;

#[test]
fn test_profile_is_localhost() {
    let p = Profile {
        server_url: "http://localhost:8889".to_string(),
        ..Default::default()
    };
    assert!(p.is_localhost());
}

#[test]
fn test_profile_is_not_localhost() {
    let p = Profile {
        server_url: "https://api.modal.com".to_string(),
        ..Default::default()
    };
    assert!(!p.is_localhost());
}

#[test]
fn test_profile_is_localhost_ipv4() {
    let p = Profile {
        server_url: "http://127.0.0.1:8889".to_string(),
        ..Default::default()
    };
    assert!(p.is_localhost());
}

#[test]
fn test_profile_is_localhost_empty_url() {
    let p = Profile {
        server_url: String::new(),
        ..Default::default()
    };
    assert!(!p.is_localhost());
}
