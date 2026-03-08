#![cfg(feature = "integration")]

mod common;

/// Integration tests for sandbox filesystem snapshots.
/// Translated from libmodal/modal-go/test/sandbox_filesystem_snapshot_test.go

#[test]
fn test_sandbox_filesystem_snapshot_create() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox, write file, snapshot, verify persistence
}

#[test]
fn test_sandbox_filesystem_snapshot_restore() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create snapshot, mount in new sandbox, verify file exists
}
