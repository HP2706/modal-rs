/// Mock infrastructure for testing gRPC services.
///
/// Provides a MockClient with FIFO handler queues per RPC method,
/// similar to the Go SDK's grpcmock package.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::error::ModalError;

/// A handler that processes a request and returns a response.
pub type UnaryHandler = Box<dyn FnOnce(Vec<u8>) -> Result<Vec<u8>, tonic::Status> + Send>;

/// MockClient wraps a client with mock capabilities for testing.
pub struct MockClient {
    method_handler_queues: Mutex<HashMap<String, Vec<UnaryHandler>>>,
}

impl MockClient {
    /// Create a new MockClient.
    pub fn new() -> Self {
        Self {
            method_handler_queues: Mutex::new(HashMap::new()),
        }
    }

    /// Register a handler for a unary RPC method.
    pub fn handle_unary<F>(&self, rpc: &str, handler: F)
    where
        F: FnOnce(Vec<u8>) -> Result<Vec<u8>, tonic::Status> + Send + 'static,
    {
        let name = short_name(rpc);
        let mut queues = self.method_handler_queues.lock().unwrap();
        queues
            .entry(name)
            .or_insert_with(Vec::new)
            .push(Box::new(handler));
    }

    /// Verify all registered mock expectations have been consumed.
    pub fn assert_exhausted(&self) -> Result<(), ModalError> {
        let queues = self.method_handler_queues.lock().unwrap();
        let mut outstanding = Vec::new();
        for (k, q) in queues.iter() {
            if !q.is_empty() {
                outstanding.push(format!("{}: {} remaining", k, q.len()));
            }
        }
        if !outstanding.is_empty() {
            return Err(ModalError::Other(format!(
                "not all expected gRPC calls were made:\n- {}",
                outstanding.join("\n- ")
            )));
        }
        Ok(())
    }

    /// Dequeue the next handler for a method.
    pub fn dequeue_handler(&self, method: &str) -> Result<UnaryHandler, ModalError> {
        let name = short_name(method);
        let mut queues = self.method_handler_queues.lock().unwrap();
        let q = queues.get_mut(&name).ok_or_else(|| {
            ModalError::Other(format!("grpcmock: unexpected gRPC call to {}", name))
        })?;
        if q.is_empty() {
            return Err(ModalError::Other(format!(
                "grpcmock: unexpected gRPC call to {}",
                name
            )));
        }
        Ok(q.remove(0))
    }
}

fn short_name(method: &str) -> String {
    if method.starts_with('/') {
        if let Some(idx) = method.rfind('/') {
            if idx + 1 < method.len() {
                return method[idx + 1..].to_string();
            }
        }
    }
    method.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_client_handle_and_dequeue() {
        let mock = MockClient::new();

        mock.handle_unary("/modal.client.ModalClient/FunctionGet", |_req| {
            Ok(vec![1, 2, 3])
        });

        let handler = mock.dequeue_handler("/modal.client.ModalClient/FunctionGet").unwrap();
        let result = handler(vec![]).unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_mock_client_assert_exhausted() {
        let mock = MockClient::new();
        assert!(mock.assert_exhausted().is_ok());

        mock.handle_unary("TestMethod", |_| Ok(vec![]));
        assert!(mock.assert_exhausted().is_err());
    }

    #[test]
    fn test_mock_client_unexpected_call() {
        let mock = MockClient::new();
        let result = mock.dequeue_handler("NonExistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_short_name() {
        assert_eq!(
            short_name("/modal.client.ModalClient/FunctionGet"),
            "FunctionGet"
        );
        assert_eq!(short_name("FunctionGet"), "FunctionGet");
    }
}
