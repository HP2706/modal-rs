#![cfg(feature = "integration")]

mod common;

/// Integration tests for sandbox directory snapshots.
/// Translated from libmodal/modal-go/test/sandbox_directory_snapshot_test.go

#[test]
fn test_sandbox_directory_mount_empty() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: mount empty directory in sandbox
}

#[test]
fn test_sandbox_directory_mount_with_image() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: mount directory with image
}

#[test]
fn test_sandbox_directory_snapshot() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: snapshot directory
}

#[test]
fn test_sandbox_directory_unbuilt_image_error() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: verify error when mounting unbuilt image
}
