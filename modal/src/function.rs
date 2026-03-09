use modal_proto::modal_proto as pb;

use crate::config::{environment_name, Profile};
use crate::error::ModalError;
use crate::invocation::{
    cbor_serialize, max_object_size_bytes, max_system_retries, BlobDownloader,
    ControlPlaneInvocation, InputPlaneInvocation, Invocation, InvocationGrpcClient,
    InvocationResult, NoBlobDownloader,
};

/// FunctionStats represents statistics for a running Function.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionStats {
    pub backlog: u32,
    pub num_total_runners: u32,
}

/// FunctionUpdateAutoscalerParams contains options for overriding a Function's autoscaler behavior.
#[derive(Debug, Clone, Default)]
pub struct FunctionUpdateAutoscalerParams {
    pub min_containers: Option<u32>,
    pub max_containers: Option<u32>,
    pub buffer_containers: Option<u32>,
    pub scaledown_window: Option<u32>,
}

/// FunctionFromNameParams are options for Functions.FromName.
#[derive(Debug, Clone, Default)]
pub struct FunctionFromNameParams {
    pub environment: String,
}

/// Function references a deployed Modal Function.
#[derive(Debug, Clone)]
pub struct Function {
    pub function_id: String,
    handle_metadata: Option<pb::FunctionHandleMetadata>,
}

impl Function {
    /// Create a new Function with the given ID and handle metadata.
    pub fn new(function_id: String, handle_metadata: Option<pb::FunctionHandleMetadata>) -> Self {
        Self {
            function_id,
            handle_metadata,
        }
    }

    /// Get the handle metadata, or return an error if not set.
    fn get_handle_metadata(&self) -> Result<&pb::FunctionHandleMetadata, ModalError> {
        self.handle_metadata.as_ref().ok_or_else(|| {
            ModalError::Other("unexpected error: function has not been hydrated".to_string())
        })
    }

    /// Get supported input formats from handle metadata.
    fn get_supported_input_formats(&self) -> Vec<i32> {
        match self.get_handle_metadata() {
            Ok(m) if !m.supported_input_formats.is_empty() => {
                m.supported_input_formats.clone()
            }
            _ => vec![],
        }
    }

    /// Get the web URL for this function, if it's a web endpoint.
    pub fn get_web_url(&self) -> String {
        match self.get_handle_metadata() {
            Ok(m) => m.web_url.clone(),
            Err(_) => String::new(),
        }
    }

    /// Check that this function is not a web endpoint (web endpoints can't be invoked via Remote/Spawn).
    fn check_no_web_url(&self, fn_name: &str) -> Result<(), ModalError> {
        let web_url = self.get_web_url();
        if !web_url.is_empty() {
            return Err(ModalError::Invalid(format!(
                "A webhook Function cannot be invoked for remote execution with '{}'. \
                 Invoke this Function via its web url '{}' instead",
                fn_name, web_url
            )));
        }
        Ok(())
    }

    /// Create a FunctionInput from args and kwargs, serialized as CBOR.
    pub fn create_input(
        &self,
        args: &[ciborium::Value],
        kwargs: &ciborium::Value,
    ) -> Result<pb::FunctionInput, ModalError> {
        // Check supported input formats and require CBOR
        let supported = self.get_supported_input_formats();
        let cbor_supported = supported
            .iter()
            .any(|f| *f == pb::DataFormat::Cbor as i32);
        if !cbor_supported {
            return Err(ModalError::Invalid(
                "cannot call Modal Function from Rust SDK since it was deployed with an \
                 incompatible Python SDK version. Redeploy with Modal Python SDK >= 1.2"
                    .to_string(),
            ));
        }

        let args_bytes = cbor_serialize(args, kwargs)?;

        let metadata = self.get_handle_metadata()?;
        let method_name = metadata.use_method_name.clone();

        // Determine if args go inline or via blob
        if args_bytes.len() > max_object_size_bytes() {
            Ok(pb::FunctionInput {
                data_format: pb::DataFormat::Cbor as i32,
                args_oneof: Some(pb::function_input::ArgsOneof::ArgsBlobId(
                    // In production, blob upload would happen here.
                    // For now, return an error - blob upload requires network.
                    return Err(ModalError::Other(
                        "function input exceeds max object size; blob upload not yet implemented"
                            .to_string(),
                    )),
                )),
                method_name: if method_name.is_empty() {
                    None
                } else {
                    Some(method_name)
                },
                ..Default::default()
            })
        } else {
            Ok(pb::FunctionInput {
                data_format: pb::DataFormat::Cbor as i32,
                args_oneof: Some(pb::function_input::ArgsOneof::Args(args_bytes)),
                method_name: if method_name.is_empty() {
                    None
                } else {
                    Some(method_name)
                },
                ..Default::default()
            })
        }
    }

    /// Remote executes a single input on a remote Function and waits for the result.
    pub fn remote<D: BlobDownloader>(
        &self,
        client: &dyn InvocationGrpcClient,
        downloader: &D,
        args: &[ciborium::Value],
        kwargs: &ciborium::Value,
    ) -> Result<InvocationResult, ModalError> {
        self.check_no_web_url("Remote")?;
        let input = self.create_input(args, kwargs)?;

        // Use control plane invocation (input plane requires separate client)
        let mut invocation = ControlPlaneInvocation::create(
            client,
            &self.function_id,
            &input,
            pb::FunctionCallInvocationType::Sync as i32,
        )?;

        let mut retry_count: u32 = 0;
        loop {
            match invocation.await_output(client, downloader, None) {
                Ok(result) => return Ok(result),
                Err(ModalError::InternalFailure(_)) if retry_count <= max_system_retries() => {
                    invocation.retry(client, retry_count)?;
                    retry_count += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// Spawn starts running a single input on a remote Function and returns a function call ID.
    pub fn spawn(
        &self,
        client: &dyn InvocationGrpcClient,
        args: &[ciborium::Value],
        kwargs: &ciborium::Value,
    ) -> Result<String, ModalError> {
        self.check_no_web_url("Spawn")?;
        let input = self.create_input(args, kwargs)?;

        let invocation = ControlPlaneInvocation::create(
            client,
            &self.function_id,
            &input,
            pb::FunctionCallInvocationType::Sync as i32,
        )?;

        Ok(invocation.function_call_id)
    }

    /// GetCurrentStats returns statistics about the Function.
    pub fn get_current_stats(
        &self,
        client: &dyn FunctionGrpcClient,
    ) -> Result<FunctionStats, ModalError> {
        client.function_get_current_stats(&self.function_id)
    }

    /// UpdateAutoscaler overrides the current autoscaler behavior for this Function.
    pub fn update_autoscaler(
        &self,
        client: &dyn FunctionGrpcClient,
        params: Option<&FunctionUpdateAutoscalerParams>,
    ) -> Result<(), ModalError> {
        let default_params = FunctionUpdateAutoscalerParams::default();
        let params = params.unwrap_or(&default_params);

        client.function_update_scheduling_params(
            &self.function_id,
            params.min_containers,
            params.max_containers,
            params.buffer_containers,
            params.scaledown_window,
        )
    }
}

/// Trait abstracting gRPC calls specific to Function operations.
pub trait FunctionGrpcClient: Send + Sync {
    /// FunctionGet retrieves a function by app name and tag.
    fn function_get(
        &self,
        app_name: &str,
        object_tag: &str,
        environment_name: &str,
    ) -> Result<pb::FunctionGetResponse, ModalError>;

    /// FunctionGetCurrentStats retrieves function statistics.
    fn function_get_current_stats(
        &self,
        function_id: &str,
    ) -> Result<FunctionStats, ModalError>;

    /// FunctionUpdateSchedulingParams updates autoscaler settings.
    fn function_update_scheduling_params(
        &self,
        function_id: &str,
        min_containers: Option<u32>,
        max_containers: Option<u32>,
        buffer_containers: Option<u32>,
        scaledown_window: Option<u32>,
    ) -> Result<(), ModalError>;
}

/// FunctionService provides Function related operations.
pub trait FunctionService: Send + Sync {
    fn from_name(
        &self,
        app_name: &str,
        name: &str,
        params: Option<&FunctionFromNameParams>,
    ) -> Result<Function, ModalError>;
}

/// Implementation of FunctionService.
pub struct FunctionServiceImpl<C: FunctionGrpcClient> {
    client: C,
    profile: Profile,
}

impl<C: FunctionGrpcClient> FunctionServiceImpl<C> {
    pub fn new(client: C, profile: Profile) -> Self {
        Self { client, profile }
    }
}

impl<C: FunctionGrpcClient> FunctionService for FunctionServiceImpl<C> {
    fn from_name(
        &self,
        app_name: &str,
        name: &str,
        params: Option<&FunctionFromNameParams>,
    ) -> Result<Function, ModalError> {
        let default_params = FunctionFromNameParams::default();
        let params = params.unwrap_or(&default_params);

        // Check for Cls method syntax
        if name.contains('.') {
            let parts: Vec<&str> = name.splitn(2, '.').collect();
            let cls_name = parts[0];
            let method_name = parts[1];
            return Err(ModalError::Invalid(format!(
                "cannot retrieve Cls methods using Functions.FromName(). Use:\n  \
                 cls = client.cls.from_name(\"{}\", \"{}\", None)\n  \
                 instance = cls.instance(None)\n  \
                 m = instance.method(\"{}\")",
                app_name, cls_name, method_name
            )));
        }

        let env = environment_name(&params.environment, &self.profile);

        let resp = self.client.function_get(app_name, name, &env)?;

        Ok(Function {
            function_id: resp.function_id,
            handle_metadata: resp.handle_metadata,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::invocation::InvocationGrpcClient;
    use std::sync::Mutex;

    // Mock for FunctionGrpcClient
    struct MockFunctionGrpcClient {
        function_get_responses: Mutex<Vec<Result<pb::FunctionGetResponse, ModalError>>>,
        get_stats_responses: Mutex<Vec<Result<FunctionStats, ModalError>>>,
        update_calls: Mutex<Vec<(String, Option<u32>, Option<u32>, Option<u32>, Option<u32>)>>,
        update_results: Mutex<Vec<Result<(), ModalError>>>,
    }

    impl MockFunctionGrpcClient {
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

    impl FunctionGrpcClient for MockFunctionGrpcClient {
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
            min_containers: Option<u32>,
            max_containers: Option<u32>,
            buffer_containers: Option<u32>,
            scaledown_window: Option<u32>,
        ) -> Result<(), ModalError> {
            self.update_calls.lock().unwrap().push((
                function_id.to_string(),
                min_containers,
                max_containers,
                buffer_containers,
                scaledown_window,
            ));
            self.update_results.lock().unwrap().remove(0)
        }
    }

    // Mock for InvocationGrpcClient (for Remote/Spawn tests)
    struct MockInvocationClient {
        function_map_responses: Mutex<Vec<Result<pb::FunctionMapResponse, ModalError>>>,
        get_outputs_responses: Mutex<Vec<Result<pb::FunctionGetOutputsResponse, ModalError>>>,
        retry_responses: Mutex<Vec<Result<pb::FunctionRetryInputsResponse, ModalError>>>,
    }

    impl MockInvocationClient {
        fn new() -> Self {
            Self {
                function_map_responses: Mutex::new(Vec::new()),
                get_outputs_responses: Mutex::new(Vec::new()),
                retry_responses: Mutex::new(Vec::new()),
            }
        }

        fn push_function_map(&self, resp: Result<pb::FunctionMapResponse, ModalError>) {
            self.function_map_responses.lock().unwrap().push(resp);
        }

        fn push_get_outputs(&self, resp: Result<pb::FunctionGetOutputsResponse, ModalError>) {
            self.get_outputs_responses.lock().unwrap().push(resp);
        }

        fn push_retry(&self, resp: Result<pb::FunctionRetryInputsResponse, ModalError>) {
            self.retry_responses.lock().unwrap().push(resp);
        }
    }

    impl InvocationGrpcClient for MockInvocationClient {
        fn function_map(
            &self,
            _function_id: &str,
            _function_call_type: i32,
            _function_call_invocation_type: i32,
            _pipelined_inputs: Vec<pb::FunctionPutInputsItem>,
        ) -> Result<pb::FunctionMapResponse, ModalError> {
            self.function_map_responses.lock().unwrap().remove(0)
        }

        fn function_get_outputs(
            &self,
            _function_call_id: &str,
            _max_values: u32,
            _timeout: f32,
            _last_entry_id: &str,
            _clear_on_success: bool,
            _requested_at: f64,
        ) -> Result<pb::FunctionGetOutputsResponse, ModalError> {
            self.get_outputs_responses.lock().unwrap().remove(0)
        }

        fn function_retry_inputs(
            &self,
            _function_call_jwt: &str,
            _inputs: Vec<pb::FunctionRetryInputsItem>,
        ) -> Result<pb::FunctionRetryInputsResponse, ModalError> {
            self.retry_responses.lock().unwrap().remove(0)
        }

        fn attempt_start(
            &self,
            _function_id: &str,
            _input: pb::FunctionPutInputsItem,
        ) -> Result<pb::AttemptStartResponse, ModalError> {
            unimplemented!()
        }

        fn attempt_await(
            &self,
            _attempt_token: &str,
            _requested_at: f64,
            _timeout_secs: f32,
        ) -> Result<pb::AttemptAwaitResponse, ModalError> {
            unimplemented!()
        }

        fn attempt_retry(
            &self,
            _function_id: &str,
            _input: pb::FunctionPutInputsItem,
            _attempt_token: &str,
        ) -> Result<pb::AttemptRetryResponse, ModalError> {
            unimplemented!()
        }

        fn blob_get(&self, _blob_id: &str) -> Result<pb::BlobGetResponse, ModalError> {
            unimplemented!()
        }
    }

    fn make_handle_metadata(supported_cbor: bool) -> pb::FunctionHandleMetadata {
        let mut formats = vec![];
        if supported_cbor {
            formats.push(pb::DataFormat::Cbor as i32);
        }
        pb::FunctionHandleMetadata {
            supported_input_formats: formats,
            ..Default::default()
        }
    }

    fn make_cbor_success_output(value: &ciborium::Value) -> pb::FunctionGetOutputsItem {
        let mut buf = Vec::new();
        ciborium::into_writer(value, &mut buf).unwrap();
        pb::FunctionGetOutputsItem {
            result: Some(pb::GenericResult {
                status: pb::generic_result::GenericStatus::Success as i32,
                data_oneof: Some(pb::generic_result::DataOneof::Data(buf)),
                ..Default::default()
            }),
            data_format: pb::DataFormat::Cbor as i32,
            ..Default::default()
        }
    }

    fn default_profile() -> Profile {
        Profile {
            environment: "main".to_string(),
            ..Default::default()
        }
    }

    // === FunctionService::from_name ===

    #[test]
    fn test_from_name_success() {
        let mock = MockFunctionGrpcClient::new();
        mock.push_function_get(Ok(pb::FunctionGetResponse {
            function_id: "fn-123".to_string(),
            handle_metadata: Some(make_handle_metadata(true)),
            ..Default::default()
        }));

        let svc = FunctionServiceImpl::new(mock, default_profile());
        let func = svc.from_name("my-app", "my-func", None).unwrap();

        assert_eq!(func.function_id, "fn-123");
        assert!(func.handle_metadata.is_some());
    }

    #[test]
    fn test_from_name_not_found() {
        let mock = MockFunctionGrpcClient::new();
        mock.push_function_get(Err(ModalError::NotFound(
            "Function 'my-app/my-func' not found".to_string(),
        )));

        let svc = FunctionServiceImpl::new(mock, default_profile());
        let err = svc.from_name("my-app", "my-func", None).unwrap_err();

        match err {
            ModalError::NotFound(msg) => assert!(msg.contains("not found")),
            other => panic!("expected NotFound, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_name_cls_method_error() {
        let mock = MockFunctionGrpcClient::new();
        let svc = FunctionServiceImpl::new(mock, default_profile());

        let err = svc.from_name("my-app", "MyClass.method", None).unwrap_err();

        match err {
            ModalError::Invalid(msg) => {
                assert!(msg.contains("Cls methods"));
                assert!(msg.contains("MyClass"));
                assert!(msg.contains("method"));
            }
            other => panic!("expected Invalid, got: {:?}", other),
        }
    }

    #[test]
    fn test_from_name_with_environment() {
        let mock = MockFunctionGrpcClient::new();
        mock.push_function_get(Ok(pb::FunctionGetResponse {
            function_id: "fn-456".to_string(),
            handle_metadata: Some(make_handle_metadata(true)),
            ..Default::default()
        }));

        let svc = FunctionServiceImpl::new(mock, default_profile());
        let params = FunctionFromNameParams {
            environment: "staging".to_string(),
        };
        let func = svc.from_name("my-app", "my-func", Some(&params)).unwrap();
        assert_eq!(func.function_id, "fn-456");
    }

    // === Function.get_web_url ===

    #[test]
    fn test_get_web_url_empty() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        assert_eq!(func.get_web_url(), "");
    }

    #[test]
    fn test_get_web_url_present() {
        let metadata = pb::FunctionHandleMetadata {
            web_url: "https://example.modal.run".to_string(),
            supported_input_formats: vec![pb::DataFormat::Cbor as i32],
            ..Default::default()
        };
        let func = Function::new("fn-1".to_string(), Some(metadata));
        assert_eq!(func.get_web_url(), "https://example.modal.run");
    }

    #[test]
    fn test_get_web_url_no_metadata() {
        let func = Function::new("fn-1".to_string(), None);
        assert_eq!(func.get_web_url(), "");
    }

    // === Function.check_no_web_url ===

    #[test]
    fn test_check_no_web_url_ok() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        assert!(func.check_no_web_url("Remote").is_ok());
    }

    #[test]
    fn test_check_no_web_url_fails() {
        let metadata = pb::FunctionHandleMetadata {
            web_url: "https://example.modal.run".to_string(),
            supported_input_formats: vec![pb::DataFormat::Cbor as i32],
            ..Default::default()
        };
        let func = Function::new("fn-1".to_string(), Some(metadata));
        let err = func.check_no_web_url("Remote").unwrap_err();

        match err {
            ModalError::Invalid(msg) => {
                assert!(msg.contains("webhook"));
                assert!(msg.contains("Remote"));
                assert!(msg.contains("https://example.modal.run"));
            }
            other => panic!("expected Invalid, got: {:?}", other),
        }
    }

    // === Function.create_input ===

    #[test]
    fn test_create_input_success() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let args = vec![ciborium::Value::Integer(1.into())];
        let kwargs = ciborium::Value::Map(vec![]);

        let input = func.create_input(&args, &kwargs).unwrap();
        assert_eq!(input.data_format, pb::DataFormat::Cbor as i32);
        assert!(input.args_oneof.is_some());
    }

    #[test]
    fn test_create_input_no_cbor_support() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(false)));
        let args = vec![];
        let kwargs = ciborium::Value::Map(vec![]);

        let err = func.create_input(&args, &kwargs).unwrap_err();
        match err {
            ModalError::Invalid(msg) => assert!(msg.contains("incompatible")),
            other => panic!("expected Invalid, got: {:?}", other),
        }
    }

    #[test]
    fn test_create_input_no_metadata() {
        let func = Function::new("fn-1".to_string(), None);
        let args = vec![];
        let kwargs = ciborium::Value::Map(vec![]);

        let err = func.create_input(&args, &kwargs).unwrap_err();
        // No metadata means no supported formats, which means CBOR not supported
        match err {
            ModalError::Invalid(msg) => assert!(msg.contains("incompatible")),
            other => panic!("expected Invalid, got: {:?}", other),
        }
    }

    #[test]
    fn test_create_input_with_method_name() {
        let metadata = pb::FunctionHandleMetadata {
            use_method_name: "my_method".to_string(),
            supported_input_formats: vec![pb::DataFormat::Cbor as i32],
            ..Default::default()
        };
        let func = Function::new("fn-1".to_string(), Some(metadata));
        let args = vec![];
        let kwargs = ciborium::Value::Map(vec![]);

        let input = func.create_input(&args, &kwargs).unwrap();
        assert_eq!(input.method_name, Some("my_method".to_string()));
    }

    #[test]
    fn test_create_input_empty_method_name() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let args = vec![];
        let kwargs = ciborium::Value::Map(vec![]);

        let input = func.create_input(&args, &kwargs).unwrap();
        assert_eq!(input.method_name, None);
    }

    // === Function.remote ===

    #[test]
    fn test_remote_success() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockInvocationClient::new();

        // FunctionMap response
        mock.push_function_map(Ok(pb::FunctionMapResponse {
            function_call_id: "fc-1".to_string(),
            function_call_jwt: "jwt-1".to_string(),
            pipelined_inputs: vec![pb::FunctionPutInputsResponseItem {
                input_jwt: "ij-1".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }));

        // FunctionGetOutputs response
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![make_cbor_success_output(&ciborium::Value::Integer(42.into()))],
            ..Default::default()
        }));

        let downloader = NoBlobDownloader;
        let result = func
            .remote(
                &mock,
                &downloader,
                &[],
                &ciborium::Value::Map(vec![]),
            )
            .unwrap();

        assert_eq!(
            result,
            InvocationResult::Cbor(ciborium::Value::Integer(42.into()))
        );
    }

    #[test]
    fn test_remote_web_url_rejected() {
        let metadata = pb::FunctionHandleMetadata {
            web_url: "https://example.modal.run".to_string(),
            supported_input_formats: vec![pb::DataFormat::Cbor as i32],
            ..Default::default()
        };
        let func = Function::new("fn-1".to_string(), Some(metadata));
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;

        let err = func
            .remote(
                &mock,
                &downloader,
                &[],
                &ciborium::Value::Map(vec![]),
            )
            .unwrap_err();

        match err {
            ModalError::Invalid(msg) => assert!(msg.contains("webhook")),
            other => panic!("expected Invalid, got: {:?}", other),
        }
    }

    #[test]
    fn test_remote_with_retry_on_internal_failure() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockInvocationClient::new();

        // FunctionMap response
        mock.push_function_map(Ok(pb::FunctionMapResponse {
            function_call_id: "fc-1".to_string(),
            function_call_jwt: "jwt-1".to_string(),
            pipelined_inputs: vec![pb::FunctionPutInputsResponseItem {
                input_jwt: "ij-1".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }));

        // First attempt: internal failure
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![pb::FunctionGetOutputsItem {
                result: Some(pb::GenericResult {
                    status: pb::generic_result::GenericStatus::InternalFailure as i32,
                    exception: "internal error".to_string(),
                    ..Default::default()
                }),
                data_format: pb::DataFormat::Cbor as i32,
                ..Default::default()
            }],
            ..Default::default()
        }));

        // Retry response
        mock.push_retry(Ok(pb::FunctionRetryInputsResponse {
            input_jwts: vec!["ij-2".to_string()],
            ..Default::default()
        }));

        // Second attempt: success
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![make_cbor_success_output(&ciborium::Value::Text(
                "retried".to_string(),
            ))],
            ..Default::default()
        }));

        let downloader = NoBlobDownloader;
        let result = func
            .remote(
                &mock,
                &downloader,
                &[],
                &ciborium::Value::Map(vec![]),
            )
            .unwrap();

        assert_eq!(
            result,
            InvocationResult::Cbor(ciborium::Value::Text("retried".to_string()))
        );
    }

    // === Function.spawn ===

    #[test]
    fn test_spawn_success() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockInvocationClient::new();

        mock.push_function_map(Ok(pb::FunctionMapResponse {
            function_call_id: "fc-spawn-1".to_string(),
            function_call_jwt: "jwt-1".to_string(),
            pipelined_inputs: vec![pb::FunctionPutInputsResponseItem {
                input_jwt: "ij-1".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }));

        let fc_id = func
            .spawn(&mock, &[], &ciborium::Value::Map(vec![]))
            .unwrap();

        assert_eq!(fc_id, "fc-spawn-1");
    }

    #[test]
    fn test_spawn_web_url_rejected() {
        let metadata = pb::FunctionHandleMetadata {
            web_url: "https://example.modal.run".to_string(),
            supported_input_formats: vec![pb::DataFormat::Cbor as i32],
            ..Default::default()
        };
        let func = Function::new("fn-1".to_string(), Some(metadata));
        let mock = MockInvocationClient::new();

        let err = func
            .spawn(&mock, &[], &ciborium::Value::Map(vec![]))
            .unwrap_err();

        match err {
            ModalError::Invalid(msg) => assert!(msg.contains("webhook")),
            other => panic!("expected Invalid, got: {:?}", other),
        }
    }

    // === Function.get_current_stats ===

    #[test]
    fn test_get_current_stats_success() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockFunctionGrpcClient::new();
        mock.push_get_stats(Ok(FunctionStats {
            backlog: 10,
            num_total_runners: 5,
        }));

        let stats = func.get_current_stats(&mock).unwrap();
        assert_eq!(stats.backlog, 10);
        assert_eq!(stats.num_total_runners, 5);
    }

    #[test]
    fn test_get_current_stats_error() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockFunctionGrpcClient::new();
        mock.push_get_stats(Err(ModalError::Other("stats failed".to_string())));

        let err = func.get_current_stats(&mock).unwrap_err();
        assert!(err.to_string().contains("stats failed"));
    }

    // === Function.update_autoscaler ===

    #[test]
    fn test_update_autoscaler_success() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockFunctionGrpcClient::new();
        mock.push_update_result(Ok(()));

        let params = FunctionUpdateAutoscalerParams {
            min_containers: Some(1),
            max_containers: Some(10),
            buffer_containers: Some(2),
            scaledown_window: Some(300),
        };

        func.update_autoscaler(&mock, Some(&params)).unwrap();

        let calls = mock.update_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "fn-1");
        assert_eq!(calls[0].1, Some(1));
        assert_eq!(calls[0].2, Some(10));
        assert_eq!(calls[0].3, Some(2));
        assert_eq!(calls[0].4, Some(300));
    }

    #[test]
    fn test_update_autoscaler_defaults() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockFunctionGrpcClient::new();
        mock.push_update_result(Ok(()));

        func.update_autoscaler(&mock, None).unwrap();

        let calls = mock.update_calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, None);
        assert_eq!(calls[0].2, None);
        assert_eq!(calls[0].3, None);
        assert_eq!(calls[0].4, None);
    }

    #[test]
    fn test_update_autoscaler_error() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let mock = MockFunctionGrpcClient::new();
        mock.push_update_result(Err(ModalError::Other("update failed".to_string())));

        let err = func.update_autoscaler(&mock, None).unwrap_err();
        assert!(err.to_string().contains("update failed"));
    }

    // === get_handle_metadata ===

    #[test]
    fn test_get_handle_metadata_present() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        assert!(func.get_handle_metadata().is_ok());
    }

    #[test]
    fn test_get_handle_metadata_missing() {
        let func = Function::new("fn-1".to_string(), None);
        let err = func.get_handle_metadata().unwrap_err();
        assert!(err.to_string().contains("not been hydrated"));
    }

    // === get_supported_input_formats ===

    #[test]
    fn test_get_supported_input_formats() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(true)));
        let formats = func.get_supported_input_formats();
        assert_eq!(formats, vec![pb::DataFormat::Cbor as i32]);
    }

    #[test]
    fn test_get_supported_input_formats_empty() {
        let func = Function::new("fn-1".to_string(), Some(make_handle_metadata(false)));
        let formats = func.get_supported_input_formats();
        assert!(formats.is_empty());
    }

    #[test]
    fn test_get_supported_input_formats_no_metadata() {
        let func = Function::new("fn-1".to_string(), None);
        let formats = func.get_supported_input_formats();
        assert!(formats.is_empty());
    }
}
