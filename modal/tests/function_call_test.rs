#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal FunctionCall.
/// Translated from libmodal/modal-go/test/function_call_test.go

#[test]
fn test_function_call_spawn_and_get() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: spawn echo_string, then get result via FunctionCalls.get
}

#[test]
fn test_function_call_get_timeout() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: spawn long-running function with short timeout
}

#[test]
fn test_function_call_cancel() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: spawn function and cancel it
}
