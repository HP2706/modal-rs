#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Volumes.
/// Translated from libmodal/modal-go/test/volume_test.go

#[test]
fn test_volume_create_and_delete() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create volume, verify exists, delete
}

#[test]
fn test_volume_ephemeral() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create ephemeral volume
}

#[test]
fn test_volume_read_only() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test read-only volume mounting
}

#[test]
fn test_volume_from_name() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: look up volume by name
}
