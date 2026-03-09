use std::time::Duration;

use crate::error::ModalError;

/// FunctionCall references a Modal Function Call. Function Calls are
/// Function invocations with a given input. They can be consumed
/// asynchronously (see get()) or cancelled (see cancel()).
#[derive(Debug, Clone)]
pub struct FunctionCall {
    pub function_call_id: String,
}

/// FunctionCallService provides FunctionCall related operations.
pub trait FunctionCallService: Send + Sync {
    fn from_id(&self, function_call_id: &str) -> Result<FunctionCall, ModalError>;
}

/// FunctionCallGetParams are options for getting outputs from Function Calls.
#[derive(Debug, Clone, Default)]
pub struct FunctionCallGetParams {
    /// Timeout specifies the maximum duration to wait for the output.
    /// If None, no timeout is applied. If set to Duration::ZERO, it will check
    /// if the function call is already completed.
    pub timeout: Option<Duration>,
}

/// FunctionCallCancelParams are options for cancelling Function Calls.
#[derive(Debug, Clone, Default)]
pub struct FunctionCallCancelParams {
    pub terminate_containers: bool,
}

/// Trait abstracting the gRPC calls needed for FunctionCall operations.
pub trait FunctionCallGrpcClient: Send + Sync {
    fn function_call_cancel(
        &self,
        function_call_id: &str,
        terminate_containers: bool,
    ) -> Result<(), ModalError>;
}

/// Implementation of FunctionCallService.
pub struct FunctionCallServiceImpl;

impl FunctionCallService for FunctionCallServiceImpl {
    fn from_id(&self, function_call_id: &str) -> Result<FunctionCall, ModalError> {
        Ok(FunctionCall {
            function_call_id: function_call_id.to_string(),
        })
    }
}

impl FunctionCall {
    /// Cancel cancels a FunctionCall.
    pub fn cancel<C: FunctionCallGrpcClient>(
        &self,
        client: &C,
        params: Option<&FunctionCallCancelParams>,
    ) -> Result<(), ModalError> {
        let default_params = FunctionCallCancelParams::default();
        let params = params.unwrap_or(&default_params);

        client
            .function_call_cancel(&self.function_call_id, params.terminate_containers)
            .map_err(|e| ModalError::Other(format!("FunctionCallCancel failed: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    struct MockFunctionCallGrpcClient {
        cancel_calls: Mutex<Vec<(String, bool)>>,
        cancel_result: Result<(), ModalError>,
    }

    impl MockFunctionCallGrpcClient {
        fn new(cancel_result: Result<(), ModalError>) -> Self {
            Self {
                cancel_calls: Mutex::new(Vec::new()),
                cancel_result,
            }
        }
    }

    impl FunctionCallGrpcClient for MockFunctionCallGrpcClient {
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
    fn test_function_call_from_id() {
        let svc = FunctionCallServiceImpl;
        let fc = svc.from_id("fc-test-123").unwrap();
        assert_eq!(fc.function_call_id, "fc-test-123");
    }

    #[test]
    fn test_function_call_cancel() {
        let mock = MockFunctionCallGrpcClient::new(Ok(()));
        let fc = FunctionCall {
            function_call_id: "fc-test-123".to_string(),
        };

        fc.cancel(&mock, None).unwrap();

        let calls = mock.cancel_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "fc-test-123");
        assert!(!calls[0].1); // terminate_containers = false by default
    }

    #[test]
    fn test_function_call_cancel_with_terminate() {
        let mock = MockFunctionCallGrpcClient::new(Ok(()));
        let fc = FunctionCall {
            function_call_id: "fc-test-456".to_string(),
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
        let mock =
            MockFunctionCallGrpcClient::new(Err(ModalError::Other("rpc failed".to_string())));
        let fc = FunctionCall {
            function_call_id: "fc-test-789".to_string(),
        };

        let err = fc.cancel(&mock, None).unwrap_err();
        assert!(
            err.to_string().contains("FunctionCallCancel failed"),
            "got: {}",
            err
        );
    }
}
