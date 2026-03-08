#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Queues.
/// Translated from libmodal/modal-go/test/queue_test.go

#[test]
fn test_queue_ephemeral_put_get() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create ephemeral queue, put/get items
}

#[test]
fn test_queue_named() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create named queue, put/get items
}

#[test]
fn test_queue_len() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test queue length
}

#[test]
fn test_queue_clear() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test queue clear
}

#[test]
fn test_queue_iterate() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test queue iteration
}

#[test]
fn test_queue_delete() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test queue deletion
}
