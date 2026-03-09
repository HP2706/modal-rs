#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Functions.
/// Translated from libmodal/modal-go/test/function_test.go

use modal::error::ModalError;
use modal::function::{
    Function, FunctionFromNameParams, FunctionGrpcClient, FunctionService, FunctionServiceImpl,
    FunctionStats, FunctionUpdateAutoscalerParams,
};
use modal_proto::modal_proto as pb;
use std::sync::Mutex;

struct MockFnGrpcClient {
    function_get_responses: Mutex<Vec<Result<pb::FunctionGetResponse, ModalError>>>,
    get_stats_responses: Mutex<Vec<Result<FunctionStats, ModalError>>>,
    update_calls: Mutex<Vec<(String, Option<u32>, Option<u32>, Option<u32>, Option<u32>)>>,
    update_results: Mutex<Vec<Result<(), ModalError>>>,
}

impl MockFnGrpcClient {
    fn new() -> Self {
        Self {
            function_get_responses: Mutex::new(Vec::new()),
            get_stats_responses: Mutex::new(Vec::new()),
            update_calls: Mutex::new(Vec::new()),
            update_results: Mutex::new(Vec::new()),
        }
    }

    fn push_function_get(&self, resp: Result<pb::FunctionGetResponse, ModalError>) {
        self.function_get_responses.lock().unwrap().push(resp);
    }

    fn push_get_stats(&self, resp: Result<FunctionStats, ModalError>) {
        self.get_stats_responses.lock().unwrap().push(resp);
    }

    fn push_update_result(&self, resp: Result<(), ModalError>) {
        self.update_results.lock().unwrap().push(resp);
    }
}

impl FunctionGrpcClient for MockFnGrpcClient {
    fn function_get(
        &self,
        _app_name: &str,
        _object_tag: &str,
        _environment_name: &str,
    ) -> Result<pb::FunctionGetResponse, ModalError> {
        self.function_get_responses.lock().unwrap().remove(0)
    }

    fn function_get_current_stats(
        &self,
        _function_id: &str,
    ) -> Result<FunctionStats, ModalError> {
        self.get_stats_responses.lock().unwrap().remove(0)
    }

    fn function_update_scheduling_params(
        &self,
        function_id: &str,
        min: Option<u32>,
        max: Option<u32>,
        buf: Option<u32>,
        scaledown: Option<u32>,
    ) -> Result<(), ModalError> {
        self.update_calls
            .lock()
            .unwrap()
            .push((function_id.to_string(), min, max, buf, scaledown));
        self.update_results.lock().unwrap().remove(0)
    }
}

fn make_metadata(cbor: bool) -> pb::FunctionHandleMetadata {
    let mut formats = vec![];
    if cbor {
        formats.push(pb::DataFormat::Cbor as i32);
    }
    pb::FunctionHandleMetadata {
        supported_input_formats: formats,
        ..Default::default()
    }
}

fn make_service(mock: MockFnGrpcClient) -> FunctionServiceImpl<MockFnGrpcClient> {
    FunctionServiceImpl::new(mock, modal::config::Profile::default())
}

#[test]
fn test_function_call() {
    let mock = MockFnGrpcClient::new();
    mock.push_function_get(Ok(pb::FunctionGetResponse {
        function_id: "fn-echo-123".to_string(),
        handle_metadata: Some(make_metadata(true)),
        ..Default::default()
    }));

    let svc = make_service(mock);
    let func = svc.from_name("my-app", "echo_string", None).unwrap();
    assert_eq!(func.function_id, "fn-echo-123");
}

#[test]
fn test_function_from_name_with_environment() {
    let mock = MockFnGrpcClient::new();
    mock.push_function_get(Ok(pb::FunctionGetResponse {
        function_id: "fn-staging-456".to_string(),
        handle_metadata: Some(make_metadata(true)),
        ..Default::default()
    }));

    let svc = make_service(mock);
    let func = svc
        .from_name(
            "my-app",
            "my-func",
            Some(&FunctionFromNameParams {
                environment: "staging".to_string(),
            }),
        )
        .unwrap();
    assert_eq!(func.function_id, "fn-staging-456");
}

#[test]
fn test_function_cls_method_error() {
    let mock = MockFnGrpcClient::new();
    let svc = make_service(mock);

    let err = svc
        .from_name("my-app", "MyClass.method", None)
        .unwrap_err();
    match err {
        ModalError::Invalid(msg) => {
            assert!(msg.contains("Cls methods"), "got: {}", msg);
            assert!(msg.contains("MyClass"), "got: {}", msg);
        }
        other => panic!("expected Invalid, got: {:?}", other),
    }
}

#[test]
fn test_function_get_current_stats() {
    let mock = MockFnGrpcClient::new();
    mock.push_get_stats(Ok(FunctionStats {
        backlog: 15,
        num_total_runners: 3,
    }));

    let func = Function::new("fn-stats-1".to_string(), Some(make_metadata(true)));
    let stats = func.get_current_stats(&mock).unwrap();
    assert_eq!(stats.backlog, 15);
    assert_eq!(stats.num_total_runners, 3);
}

#[test]
fn test_function_update_autoscaler() {
    let mock = MockFnGrpcClient::new();
    mock.push_update_result(Ok(()));

    let func = Function::new("fn-auto-1".to_string(), Some(make_metadata(true)));
    let params = FunctionUpdateAutoscalerParams {
        min_containers: Some(1),
        max_containers: Some(10),
        buffer_containers: Some(2),
        scaledown_window: Some(300),
    };

    func.update_autoscaler(&mock, Some(&params)).unwrap();

    let calls = mock.update_calls.lock().unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, "fn-auto-1");
    assert_eq!(calls[0].1, Some(1));
    assert_eq!(calls[0].2, Some(10));
    assert_eq!(calls[0].3, Some(2));
    assert_eq!(calls[0].4, Some(300));
}

#[test]
fn test_function_web_endpoint() {
    let metadata = pb::FunctionHandleMetadata {
        web_url: "https://my-app.modal.run".to_string(),
        supported_input_formats: vec![pb::DataFormat::Cbor as i32],
        ..Default::default()
    };
    let func = Function::new("fn-web-1".to_string(), Some(metadata));

    assert_eq!(func.get_web_url(), "https://my-app.modal.run");
}

#[test]
fn test_function_create_input_cbor() {
    let func = Function::new("fn-1".to_string(), Some(make_metadata(true)));
    let args = vec![ciborium::Value::Text("hello".to_string())];
    let kwargs = ciborium::Value::Map(vec![]);

    let input = func.create_input(&args, &kwargs).unwrap();
    assert_eq!(input.data_format, pb::DataFormat::Cbor as i32);
    assert!(input.args_oneof.is_some());
}

#[test]
fn test_function_create_input_no_cbor_support() {
    let func = Function::new("fn-1".to_string(), Some(make_metadata(false)));
    let args = vec![];
    let kwargs = ciborium::Value::Map(vec![]);

    let err = func.create_input(&args, &kwargs).unwrap_err();
    match err {
        ModalError::Invalid(msg) => assert!(msg.contains("incompatible")),
        other => panic!("expected Invalid, got: {:?}", other),
    }
}
