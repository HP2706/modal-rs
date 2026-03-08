#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Functions.
/// Translated from libmodal/modal-go/test/function_test.go

#[test]
fn test_function_call() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();

    // TODO: Implement when FunctionService.from_name is wired to gRPC
    // let function = client.functions.from_name("libmodal-test-support", "echo_string", None).await.unwrap();
    // let result = function.remote(None, &[("s", "hello")]).await.unwrap();
    // assert_eq!(result, "output: hello");
}

#[test]
fn test_function_call_with_datetime_roundtrip() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: Implement datetime roundtrip test
}

#[test]
fn test_function_call_with_large_input() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: Implement large input test (>4MB to trigger blob upload)
}

#[test]
fn test_function_spawn() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: Implement spawn test
}

#[test]
fn test_function_get_current_stats() {
    skip_if_no_credentials!();
    // TODO: Implement with mock client
    // Uses grpc_mock to test FunctionGetCurrentStats
}

#[test]
fn test_function_web_endpoint() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: Implement web endpoint test
}
