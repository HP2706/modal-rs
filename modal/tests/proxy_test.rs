#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Proxies.
/// Translated from libmodal/modal-go/test/proxy_test.go

#[test]
fn test_proxy_create() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create proxy and use it with sandbox
}

#[test]
fn test_proxy_not_found() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test error when proxy not found
}
