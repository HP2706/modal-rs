#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal FunctionCall.
/// Translated from libmodal/modal-go/test/function_call_test.go

use modal::error::ModalError;
use modal::function_call::{
    FunctionCall, FunctionCallCancelParams, FunctionCallGrpcClient, FunctionCallService,
    FunctionCallServiceImpl,
};
use std::sync::Mutex;

struct MockFunctionCallClient {
    cancel_calls: Mutex<Vec<(String, bool)>>,
    cancel_result: Result<(), ModalError>,
}

impl MockFunctionCallClient {
    fn new(cancel_result: Result<(), ModalError>) -> Self {
        Self {
            cancel_calls: Mutex::new(Vec::new()),
            cancel_result,
        }
    }
}

impl FunctionCallGrpcClient for MockFunctionCallClient {
    fn function_call_cancel(
        &self,
        function_call_id: &str,
        terminate_containers: bool,
    ) -> Result<(), ModalError> {
        self.cancel_calls
            .lock()
            .unwrap()
            .push((function_call_id.to_string(), terminate_containers));
        match &self.cancel_result {
            Ok(()) => Ok(()),
            Err(e) => Err(ModalError::Other(e.to_string())),
        }
    }
}

#[test]
fn test_function_call_spawn_and_get() {
    // Test FunctionCall creation via FromID and field access
    let svc = FunctionCallServiceImpl;
    let fc = svc.from_id("fc-spawn-123").unwrap();
    assert_eq!(fc.function_call_id, "fc-spawn-123");
}

#[test]
fn test_function_call_from_id_various() {
    let svc = FunctionCallServiceImpl;
    for id in &["fc-abc", "fc-123-456-789", "fc-long-id-string"] {
        let fc = svc.from_id(id).unwrap();
        assert_eq!(fc.function_call_id, *id);
    }
}

#[test]
fn test_function_call_cancel() {
    let mock = MockFunctionCallClient::new(Ok(()));
    let fc = FunctionCall {
        function_call_id: "fc-cancel-1".to_string(),
    };

    fc.cancel(&mock, None).unwrap();

    let calls = mock.cancel_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "fc-cancel-1");
    assert!(!calls[0].1); // terminate_containers defaults to false
}

#[test]
fn test_function_call_cancel_with_terminate() {
    let mock = MockFunctionCallClient::new(Ok(()));
    let fc = FunctionCall {
        function_call_id: "fc-cancel-2".to_string(),
    };

    fc.cancel(
        &mock,
        Some(&FunctionCallCancelParams {
            terminate_containers: true,
        }),
    )
    .unwrap();

    let calls = mock.cancel_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert!(calls[0].1);
}

#[test]
fn test_function_call_cancel_error() {
    let mock = MockFunctionCallClient::new(Err(ModalError::Other("rpc failed".to_string())));
    let fc = FunctionCall {
        function_call_id: "fc-cancel-3".to_string(),
    };

    let err = fc.cancel(&mock, None).unwrap_err();
    assert!(
        err.to_string().contains("FunctionCallCancel failed"),
        "got: {}",
        err
    );
}
