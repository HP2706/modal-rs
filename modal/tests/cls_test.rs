#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Cls.
/// Translated from libmodal/modal-go/test/cls_test.go

#[test]
fn test_cls_from_name() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: look up Cls by name and invoke a method
}

#[test]
fn test_cls_with_parameters() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test parametrized class instantiation
}
