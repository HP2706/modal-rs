#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Secrets.
/// Translated from libmodal/modal-go/test/secret_test.go

#[test]
fn test_secret_from_name() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: look up secret by name
}

#[test]
fn test_secret_from_map() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create secret from map
}

#[test]
fn test_secret_delete() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create and delete secret
}

#[test]
fn test_secret_required_keys() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test required keys validation
}
