use std::time::{Duration, Instant};

use modal_proto::modal_proto as pb;
use crate::error::ModalError;

/// From: modal-client/modal/_utils/function_utils.py
/// Polling timeout for FunctionGetOutputs — refresh backend call every 55 seconds.
const OUTPUTS_TIMEOUT: Duration = Duration::from_secs(55);

/// From: modal/_functions.py
const MAX_SYSTEM_RETRIES: u32 = 8;

/// From: modal/_utils/blob_utils.py
const MAX_OBJECT_SIZE_BYTES: usize = 2 * 1024 * 1024; // 2 MiB

fn time_now_seconds() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

/// Trait abstracting the gRPC calls needed for invocation operations.
/// Separated for testability — production code uses a real gRPC client.
pub trait InvocationGrpcClient: Send + Sync {
    /// FunctionMap creates a function call and returns the response.
    fn function_map(
        &self,
        function_id: &str,
        function_call_type: i32,
        function_call_invocation_type: i32,
        pipelined_inputs: Vec<pb::FunctionPutInputsItem>,
    ) -> Result<pb::FunctionMapResponse, ModalError>;

    /// FunctionGetOutputs polls for function outputs.
    fn function_get_outputs(
        &self,
        function_call_id: &str,
        max_values: u32,
        timeout: f32,
        last_entry_id: &str,
        clear_on_success: bool,
        requested_at: f64,
    ) -> Result<pb::FunctionGetOutputsResponse, ModalError>;

    /// FunctionRetryInputs retries a function input.
    fn function_retry_inputs(
        &self,
        function_call_jwt: &str,
        inputs: Vec<pb::FunctionRetryInputsItem>,
    ) -> Result<pb::FunctionRetryInputsResponse, ModalError>;

    /// AttemptStart starts an input-plane attempt.
    fn attempt_start(
        &self,
        function_id: &str,
        input: pb::FunctionPutInputsItem,
    ) -> Result<pb::AttemptStartResponse, ModalError>;

    /// AttemptAwait waits for an input-plane attempt result.
    fn attempt_await(
        &self,
        attempt_token: &str,
        requested_at: f64,
        timeout_secs: f32,
    ) -> Result<pb::AttemptAwaitResponse, ModalError>;

    /// AttemptRetry retries an input-plane attempt.
    fn attempt_retry(
        &self,
        function_id: &str,
        input: pb::FunctionPutInputsItem,
        attempt_token: &str,
    ) -> Result<pb::AttemptRetryResponse, ModalError>;

    /// BlobGet gets a blob download URL.
    fn blob_get(&self, blob_id: &str) -> Result<pb::BlobGetResponse, ModalError>;
}

/// Trait for downloading blobs via HTTP. Separated so tests can mock it.
pub trait BlobDownloader: Send + Sync {
    fn download(&self, url: &str) -> Result<Vec<u8>, ModalError>;
}

/// A no-op blob downloader that returns an error (for use in test contexts
/// where blob downloads aren't expected).
pub struct NoBlobDownloader;

impl BlobDownloader for NoBlobDownloader {
    fn download(&self, _url: &str) -> Result<Vec<u8>, ModalError> {
        Err(ModalError::Other(
            "blob download not supported in this context".to_string(),
        ))
    }
}

/// Invocation trait — the core abstraction for function call execution.
pub trait Invocation: Send + Sync {
    /// Wait for the function output, with an optional timeout.
    fn await_output<D: BlobDownloader>(
        &self,
        client: &dyn InvocationGrpcClient,
        downloader: &D,
        timeout: Option<Duration>,
    ) -> Result<InvocationResult, ModalError>;

    /// Retry the invocation.
    fn retry(&mut self, client: &dyn InvocationGrpcClient, retry_count: u32)
        -> Result<(), ModalError>;
}

/// Result from a successful invocation.
#[derive(Debug, Clone, PartialEq)]
pub enum InvocationResult {
    /// CBOR-deserialized value (represented as ciborium::Value for generality).
    Cbor(ciborium::Value),
    /// GeneratorDone signal.
    GeneratorDone(Vec<u8>),
    /// Null / empty result.
    Null,
}

/// Control plane invocation — routes through the Modal control plane.
#[derive(Debug)]
pub struct ControlPlaneInvocation {
    pub function_call_id: String,
    input: Option<pb::FunctionInput>,
    function_call_jwt: String,
    input_jwt: String,
}

impl ControlPlaneInvocation {
    /// Create a control plane invocation by calling FunctionMap.
    pub fn create(
        client: &dyn InvocationGrpcClient,
        function_id: &str,
        input: &pb::FunctionInput,
        invocation_type: i32,
    ) -> Result<Self, ModalError> {
        let item = pb::FunctionPutInputsItem {
            idx: 0,
            input: Some(input.clone()),
            ..Default::default()
        };

        let response = client.function_map(
            function_id,
            pb::FunctionCallType::Unary as i32,
            invocation_type,
            vec![item],
        )?;

        let input_jwt = response
            .pipelined_inputs
            .first()
            .map(|i| i.input_jwt.clone())
            .unwrap_or_default();

        Ok(Self {
            function_call_id: response.function_call_id,
            input: Some(input.clone()),
            function_call_jwt: response.function_call_jwt,
            input_jwt,
        })
    }

    /// Create from an existing function call ID (for FunctionCall.Get).
    pub fn from_function_call_id(function_call_id: String) -> Self {
        Self {
            function_call_id,
            input: None,
            function_call_jwt: String::new(),
            input_jwt: String::new(),
        }
    }

    fn get_output(
        &self,
        client: &dyn InvocationGrpcClient,
        timeout: Duration,
    ) -> Result<Option<pb::FunctionGetOutputsItem>, ModalError> {
        let response = client.function_get_outputs(
            &self.function_call_id,
            1,
            timeout.as_secs_f32(),
            "0-0",
            true,
            time_now_seconds(),
        )?;

        if let Some(output) = response.outputs.into_iter().next() {
            Ok(Some(output))
        } else {
            Ok(None)
        }
    }
}

impl Invocation for ControlPlaneInvocation {
    fn await_output<D: BlobDownloader>(
        &self,
        client: &dyn InvocationGrpcClient,
        downloader: &D,
        timeout: Option<Duration>,
    ) -> Result<InvocationResult, ModalError> {
        poll_function_output(client, downloader, |c, t| self.get_output(c, t), timeout)
    }

    fn retry(
        &mut self,
        client: &dyn InvocationGrpcClient,
        retry_count: u32,
    ) -> Result<(), ModalError> {
        let input = self.input.as_ref().ok_or_else(|| {
            ModalError::Other("cannot retry Function invocation - input missing".to_string())
        })?;

        let retry_item = pb::FunctionRetryInputsItem {
            input_jwt: self.input_jwt.clone(),
            input: Some(input.clone()),
            retry_count,
        };

        let response =
            client.function_retry_inputs(&self.function_call_jwt, vec![retry_item])?;

        if let Some(jwt) = response.input_jwts.first() {
            self.input_jwt = jwt.clone();
        }

        Ok(())
    }
}

/// Input plane invocation — routes through the Modal input plane for lower latency.
#[derive(Debug)]
pub struct InputPlaneInvocation {
    function_id: String,
    input: pb::FunctionPutInputsItem,
    attempt_token: String,
}

impl InputPlaneInvocation {
    /// Create an input plane invocation by calling AttemptStart.
    pub fn create(
        client: &dyn InvocationGrpcClient,
        function_id: &str,
        input: &pb::FunctionInput,
    ) -> Result<Self, ModalError> {
        let item = pb::FunctionPutInputsItem {
            idx: 0,
            input: Some(input.clone()),
            ..Default::default()
        };

        let response = client.attempt_start(function_id, item.clone())?;

        Ok(Self {
            function_id: function_id.to_string(),
            input: item,
            attempt_token: response.attempt_token,
        })
    }

    fn get_output(
        &self,
        client: &dyn InvocationGrpcClient,
        timeout: Duration,
    ) -> Result<Option<pb::FunctionGetOutputsItem>, ModalError> {
        let resp = client.attempt_await(
            &self.attempt_token,
            time_now_seconds(),
            timeout.as_secs_f32(),
        )?;

        Ok(resp.output)
    }
}

impl Invocation for InputPlaneInvocation {
    fn await_output<D: BlobDownloader>(
        &self,
        client: &dyn InvocationGrpcClient,
        downloader: &D,
        timeout: Option<Duration>,
    ) -> Result<InvocationResult, ModalError> {
        poll_function_output(client, downloader, |c, t| self.get_output(c, t), timeout)
    }

    fn retry(
        &mut self,
        client: &dyn InvocationGrpcClient,
        _retry_count: u32,
    ) -> Result<(), ModalError> {
        let resp = client.attempt_retry(
            &self.function_id,
            self.input.clone(),
            &self.attempt_token,
        )?;

        self.attempt_token = resp.attempt_token;
        Ok(())
    }
}

/// Poll for function output, retrying on empty results until timeout.
fn poll_function_output<F, D: BlobDownloader>(
    client: &dyn InvocationGrpcClient,
    downloader: &D,
    get_output: F,
    timeout: Option<Duration>,
) -> Result<InvocationResult, ModalError>
where
    F: Fn(&dyn InvocationGrpcClient, Duration) -> Result<Option<pb::FunctionGetOutputsItem>, ModalError>,
{
    let start = Instant::now();
    let mut poll_timeout = match &timeout {
        Some(t) => std::cmp::min(*t, OUTPUTS_TIMEOUT),
        None => OUTPUTS_TIMEOUT,
    };

    loop {
        let output = get_output(client, poll_timeout)?;

        if let Some(item) = output {
            return process_result(client, downloader, item.result.as_ref(), item.data_format);
        }

        if let Some(total_timeout) = &timeout {
            let elapsed = start.elapsed();
            if elapsed >= *total_timeout {
                return Err(ModalError::FunctionTimeout(format!(
                    "Timeout exceeded: {:.1}s",
                    total_timeout.as_secs_f64()
                )));
            }
            let remaining = *total_timeout - elapsed;
            poll_timeout = std::cmp::min(OUTPUTS_TIMEOUT, remaining);
        }
    }
}

/// Process the result from an invocation output.
fn process_result<D: BlobDownloader>(
    client: &dyn InvocationGrpcClient,
    downloader: &D,
    result: Option<&pb::GenericResult>,
    data_format: i32,
) -> Result<InvocationResult, ModalError> {
    let result = result.ok_or_else(|| {
        ModalError::Remote("Received null result from invocation".to_string())
    })?;

    // Extract data from either inline or blob
    let data: Option<Vec<u8>> = match &result.data_oneof {
        Some(pb::generic_result::DataOneof::Data(d)) => Some(d.clone()),
        Some(pb::generic_result::DataOneof::DataBlobId(blob_id)) => {
            Some(blob_download(client, downloader, blob_id)?)
        }
        None => None,
    };

    // Check status
    let status = result.status;
    if status == pb::generic_result::GenericStatus::Timeout as i32 {
        return Err(ModalError::FunctionTimeout(result.exception.clone()));
    }
    if status == pb::generic_result::GenericStatus::InternalFailure as i32 {
        return Err(ModalError::InternalFailure(result.exception.clone()));
    }
    if status != pb::generic_result::GenericStatus::Success as i32 {
        // Non-success: could be Failure, Terminated, etc.
        return Err(ModalError::Remote(result.exception.clone()));
    }

    // Success — deserialize according to data format
    deserialize_data_format(data.as_deref(), data_format)
}

/// Download a blob by its ID using the gRPC client to get the URL, then HTTP to download.
fn blob_download<D: BlobDownloader>(
    client: &dyn InvocationGrpcClient,
    downloader: &D,
    blob_id: &str,
) -> Result<Vec<u8>, ModalError> {
    let resp = client.blob_get(blob_id)?;
    downloader.download(&resp.download_url)
}

/// Deserialize data according to its DataFormat.
fn deserialize_data_format(
    data: Option<&[u8]>,
    data_format: i32,
) -> Result<InvocationResult, ModalError> {
    if data_format == pb::DataFormat::Cbor as i32 {
        match data {
            Some(bytes) if !bytes.is_empty() => {
                let value: ciborium::Value = ciborium::from_reader(bytes).map_err(|e| {
                    ModalError::Serialization(format!("failed to decode CBOR: {}", e))
                })?;
                Ok(InvocationResult::Cbor(value))
            }
            _ => Ok(InvocationResult::Null),
        }
    } else if data_format == pb::DataFormat::Pickle as i32 {
        Err(ModalError::Serialization(
            "PICKLE output format is not supported - remote function must return CBOR format"
                .to_string(),
        ))
    } else if data_format == pb::DataFormat::Asgi as i32 {
        Err(ModalError::Serialization(
            "ASGI data format is not supported in Rust".to_string(),
        ))
    } else if data_format == pb::DataFormat::GeneratorDone as i32 {
        match data {
            Some(bytes) => Ok(InvocationResult::GeneratorDone(bytes.to_vec())),
            None => Ok(InvocationResult::GeneratorDone(Vec::new())),
        }
    } else {
        Err(ModalError::Serialization(format!(
            "unsupported data format: {}",
            data_format
        )))
    }
}

/// Serialize arguments to CBOR for function input.
pub fn cbor_serialize(args: &[ciborium::Value], kwargs: &ciborium::Value) -> Result<Vec<u8>, ModalError> {
    let payload = ciborium::Value::Array(vec![
        ciborium::Value::Array(args.to_vec()),
        kwargs.clone(),
    ]);
    let mut buf = Vec::new();
    ciborium::into_writer(&payload, &mut buf)
        .map_err(|e| ModalError::Serialization(format!("failed to encode CBOR: {}", e)))?;
    Ok(buf)
}

/// Deserialize CBOR bytes into a ciborium::Value.
pub fn cbor_deserialize(data: &[u8]) -> Result<ciborium::Value, ModalError> {
    ciborium::from_reader(data)
        .map_err(|e| ModalError::Serialization(format!("failed to decode CBOR: {}", e)))
}

/// Return the maximum system retries constant.
pub fn max_system_retries() -> u32 {
    MAX_SYSTEM_RETRIES
}

/// Return the max object size for inline data (above this, use blob upload).
pub fn max_object_size_bytes() -> usize {
    MAX_OBJECT_SIZE_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock gRPC client for testing invocation operations.
    struct MockInvocationClient {
        function_map_responses: Mutex<Vec<Result<pb::FunctionMapResponse, ModalError>>>,
        get_outputs_responses:
            Mutex<Vec<Result<pb::FunctionGetOutputsResponse, ModalError>>>,
        retry_responses:
            Mutex<Vec<Result<pb::FunctionRetryInputsResponse, ModalError>>>,
        attempt_start_responses: Mutex<Vec<Result<pb::AttemptStartResponse, ModalError>>>,
        attempt_await_responses: Mutex<Vec<Result<pb::AttemptAwaitResponse, ModalError>>>,
        attempt_retry_responses: Mutex<Vec<Result<pb::AttemptRetryResponse, ModalError>>>,
        blob_get_responses: Mutex<Vec<Result<pb::BlobGetResponse, ModalError>>>,
    }

    impl MockInvocationClient {
        fn new() -> Self {
            Self {
                function_map_responses: Mutex::new(Vec::new()),
                get_outputs_responses: Mutex::new(Vec::new()),
                retry_responses: Mutex::new(Vec::new()),
                attempt_start_responses: Mutex::new(Vec::new()),
                attempt_await_responses: Mutex::new(Vec::new()),
                attempt_retry_responses: Mutex::new(Vec::new()),
                blob_get_responses: Mutex::new(Vec::new()),
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

        fn push_attempt_start(&self, resp: Result<pb::AttemptStartResponse, ModalError>) {
            self.attempt_start_responses.lock().unwrap().push(resp);
        }

        fn push_attempt_await(&self, resp: Result<pb::AttemptAwaitResponse, ModalError>) {
            self.attempt_await_responses.lock().unwrap().push(resp);
        }

        fn push_attempt_retry(&self, resp: Result<pb::AttemptRetryResponse, ModalError>) {
            self.attempt_retry_responses.lock().unwrap().push(resp);
        }

        fn push_blob_get(&self, resp: Result<pb::BlobGetResponse, ModalError>) {
            self.blob_get_responses.lock().unwrap().push(resp);
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
            self.function_map_responses
                .lock()
                .unwrap()
                .remove(0)
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
            self.attempt_start_responses.lock().unwrap().remove(0)
        }

        fn attempt_await(
            &self,
            _attempt_token: &str,
            _requested_at: f64,
            _timeout_secs: f32,
        ) -> Result<pb::AttemptAwaitResponse, ModalError> {
            self.attempt_await_responses.lock().unwrap().remove(0)
        }

        fn attempt_retry(
            &self,
            _function_id: &str,
            _input: pb::FunctionPutInputsItem,
            _attempt_token: &str,
        ) -> Result<pb::AttemptRetryResponse, ModalError> {
            self.attempt_retry_responses.lock().unwrap().remove(0)
        }

        fn blob_get(&self, _blob_id: &str) -> Result<pb::BlobGetResponse, ModalError> {
            self.blob_get_responses.lock().unwrap().remove(0)
        }
    }

    struct MockBlobDownloader {
        responses: Mutex<Vec<Result<Vec<u8>, ModalError>>>,
    }

    impl MockBlobDownloader {
        fn new() -> Self {
            Self {
                responses: Mutex::new(Vec::new()),
            }
        }

        fn push(&self, resp: Result<Vec<u8>, ModalError>) {
            self.responses.lock().unwrap().push(resp);
        }
    }

    impl BlobDownloader for MockBlobDownloader {
        fn download(&self, _url: &str) -> Result<Vec<u8>, ModalError> {
            self.responses.lock().unwrap().remove(0)
        }
    }

    fn make_cbor_bytes(value: &ciborium::Value) -> Vec<u8> {
        let mut buf = Vec::new();
        ciborium::into_writer(value, &mut buf).unwrap();
        buf
    }

    fn make_success_output(data: Vec<u8>, data_format: i32) -> pb::FunctionGetOutputsItem {
        pb::FunctionGetOutputsItem {
            result: Some(pb::GenericResult {
                status: pb::generic_result::GenericStatus::Success as i32,
                data_oneof: Some(pb::generic_result::DataOneof::Data(data)),
                ..Default::default()
            }),
            data_format,
            ..Default::default()
        }
    }

    fn make_function_input() -> pb::FunctionInput {
        let cbor_data = make_cbor_bytes(&ciborium::Value::Array(vec![
            ciborium::Value::Array(vec![]),
            ciborium::Value::Map(vec![]),
        ]));
        pb::FunctionInput {
            data_format: pb::DataFormat::Cbor as i32,
            args_oneof: Some(pb::function_input::ArgsOneof::Args(cbor_data)),
            ..Default::default()
        }
    }

    // === ControlPlaneInvocation Tests ===

    #[test]
    fn test_control_plane_create() {
        let mock = MockInvocationClient::new();
        mock.push_function_map(Ok(pb::FunctionMapResponse {
            function_call_id: "fc-123".to_string(),
            function_call_jwt: "jwt-abc".to_string(),
            pipelined_inputs: vec![pb::FunctionPutInputsResponseItem {
                input_jwt: "input-jwt-1".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }));

        let input = make_function_input();
        let inv = ControlPlaneInvocation::create(
            &mock,
            "fn-1",
            &input,
            pb::FunctionCallInvocationType::Sync as i32,
        )
        .unwrap();

        assert_eq!(inv.function_call_id, "fc-123");
        assert_eq!(inv.function_call_jwt, "jwt-abc");
        assert_eq!(inv.input_jwt, "input-jwt-1");
        assert!(inv.input.is_some());
    }

    #[test]
    fn test_control_plane_create_error() {
        let mock = MockInvocationClient::new();
        mock.push_function_map(Err(ModalError::Other("rpc failed".to_string())));

        let input = make_function_input();
        let err = ControlPlaneInvocation::create(
            &mock,
            "fn-1",
            &input,
            pb::FunctionCallInvocationType::Sync as i32,
        )
        .unwrap_err();

        assert!(err.to_string().contains("rpc failed"));
    }

    #[test]
    fn test_control_plane_from_function_call_id() {
        let inv = ControlPlaneInvocation::from_function_call_id("fc-existing".to_string());
        assert_eq!(inv.function_call_id, "fc-existing");
        assert!(inv.input.is_none());
    }

    #[test]
    fn test_control_plane_await_output_success_cbor() {
        let mock = MockInvocationClient::new();
        let cbor_data = make_cbor_bytes(&ciborium::Value::Integer(42.into()));
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![make_success_output(cbor_data, pb::DataFormat::Cbor as i32)],
            ..Default::default()
        }));

        let inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let downloader = NoBlobDownloader;
        let result = inv.await_output(&mock, &downloader, None).unwrap();

        assert_eq!(result, InvocationResult::Cbor(ciborium::Value::Integer(42.into())));
    }

    #[test]
    fn test_control_plane_await_output_timeout_error() {
        let mock = MockInvocationClient::new();
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![pb::FunctionGetOutputsItem {
                result: Some(pb::GenericResult {
                    status: pb::generic_result::GenericStatus::Timeout as i32,
                    exception: "function timed out".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }));

        let inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let downloader = NoBlobDownloader;
        let err = inv.await_output(&mock, &downloader, None).unwrap_err();

        match err {
            ModalError::FunctionTimeout(msg) => assert!(msg.contains("function timed out")),
            other => panic!("expected FunctionTimeout, got: {:?}", other),
        }
    }

    #[test]
    fn test_control_plane_await_output_remote_error() {
        let mock = MockInvocationClient::new();
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![pb::FunctionGetOutputsItem {
                result: Some(pb::GenericResult {
                    status: pb::generic_result::GenericStatus::Failure as i32,
                    exception: "user code error".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }));

        let inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let downloader = NoBlobDownloader;
        let err = inv.await_output(&mock, &downloader, None).unwrap_err();

        match err {
            ModalError::Remote(msg) => assert!(msg.contains("user code error")),
            other => panic!("expected Remote, got: {:?}", other),
        }
    }

    #[test]
    fn test_control_plane_await_output_internal_failure() {
        let mock = MockInvocationClient::new();
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![pb::FunctionGetOutputsItem {
                result: Some(pb::GenericResult {
                    status: pb::generic_result::GenericStatus::InternalFailure as i32,
                    exception: "internal error".to_string(),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }));

        let inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let downloader = NoBlobDownloader;
        let err = inv.await_output(&mock, &downloader, None).unwrap_err();

        match err {
            ModalError::InternalFailure(msg) => assert!(msg.contains("internal error")),
            other => panic!("expected InternalFailure, got: {:?}", other),
        }
    }

    #[test]
    fn test_control_plane_await_output_null_result() {
        let mock = MockInvocationClient::new();
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![pb::FunctionGetOutputsItem {
                result: None,
                ..Default::default()
            }],
            ..Default::default()
        }));

        let inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let downloader = NoBlobDownloader;
        let err = inv.await_output(&mock, &downloader, None).unwrap_err();

        match err {
            ModalError::Remote(msg) => assert!(msg.contains("null result")),
            other => panic!("expected Remote, got: {:?}", other),
        }
    }

    #[test]
    fn test_control_plane_await_output_polls_until_result() {
        let mock = MockInvocationClient::new();
        // First poll: empty
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![],
            ..Default::default()
        }));
        // Second poll: result
        let cbor_data = make_cbor_bytes(&ciborium::Value::Text("hello".to_string()));
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![make_success_output(cbor_data, pb::DataFormat::Cbor as i32)],
            ..Default::default()
        }));

        let inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let downloader = NoBlobDownloader;
        let result = inv.await_output(&mock, &downloader, None).unwrap();

        assert_eq!(
            result,
            InvocationResult::Cbor(ciborium::Value::Text("hello".to_string()))
        );
    }

    #[test]
    fn test_control_plane_await_output_with_timeout_expired() {
        let mock = MockInvocationClient::new();
        // Return empty results so polling continues until timeout
        mock.push_get_outputs(Ok(pb::FunctionGetOutputsResponse {
            outputs: vec![],
            ..Default::default()
        }));

        let inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let downloader = NoBlobDownloader;
        let err = inv
            .await_output(&mock, &downloader, Some(Duration::from_millis(0)))
            .unwrap_err();

        match err {
            ModalError::FunctionTimeout(msg) => assert!(msg.contains("Timeout exceeded")),
            other => panic!("expected FunctionTimeout, got: {:?}", other),
        }
    }

    #[test]
    fn test_control_plane_retry() {
        let mock = MockInvocationClient::new();
        mock.push_function_map(Ok(pb::FunctionMapResponse {
            function_call_id: "fc-123".to_string(),
            function_call_jwt: "jwt-abc".to_string(),
            pipelined_inputs: vec![pb::FunctionPutInputsResponseItem {
                input_jwt: "input-jwt-1".to_string(),
                ..Default::default()
            }],
            ..Default::default()
        }));
        mock.push_retry(Ok(pb::FunctionRetryInputsResponse {
            input_jwts: vec!["input-jwt-2".to_string()],
            ..Default::default()
        }));

        let input = make_function_input();
        let mut inv = ControlPlaneInvocation::create(
            &mock,
            "fn-1",
            &input,
            pb::FunctionCallInvocationType::Sync as i32,
        )
        .unwrap();

        inv.retry(&mock, 1).unwrap();
        assert_eq!(inv.input_jwt, "input-jwt-2");
    }

    #[test]
    fn test_control_plane_retry_without_input() {
        let mut inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let mock = MockInvocationClient::new();

        let err = inv.retry(&mock, 0);
        assert!(err.is_err());
    }

    // === InputPlaneInvocation Tests ===

    #[test]
    fn test_input_plane_create() {
        let mock = MockInvocationClient::new();
        mock.push_attempt_start(Ok(pb::AttemptStartResponse {
            attempt_token: "attempt-token-1".to_string(),
            ..Default::default()
        }));

        let input = make_function_input();
        let inv = InputPlaneInvocation::create(&mock, "fn-2", &input).unwrap();

        assert_eq!(inv.function_id, "fn-2");
        assert_eq!(inv.attempt_token, "attempt-token-1");
    }

    #[test]
    fn test_input_plane_create_error() {
        let mock = MockInvocationClient::new();
        mock.push_attempt_start(Err(ModalError::Other("attempt failed".to_string())));

        let input = make_function_input();
        let err = InputPlaneInvocation::create(&mock, "fn-2", &input).unwrap_err();
        assert!(err.to_string().contains("attempt failed"));
    }

    #[test]
    fn test_input_plane_await_output_success() {
        let mock = MockInvocationClient::new();
        let cbor_data = make_cbor_bytes(&ciborium::Value::Bool(true));
        mock.push_attempt_await(Ok(pb::AttemptAwaitResponse {
            output: Some(make_success_output(cbor_data, pb::DataFormat::Cbor as i32)),
            ..Default::default()
        }));

        let input = make_function_input();
        let item = pb::FunctionPutInputsItem {
            idx: 0,
            input: Some(input),
            ..Default::default()
        };
        let inv = InputPlaneInvocation {
            function_id: "fn-2".to_string(),
            input: item,
            attempt_token: "tok-1".to_string(),
        };

        let downloader = NoBlobDownloader;
        let result = inv.await_output(&mock, &downloader, None).unwrap();
        assert_eq!(result, InvocationResult::Cbor(ciborium::Value::Bool(true)));
    }

    #[test]
    fn test_input_plane_retry() {
        let mock = MockInvocationClient::new();
        mock.push_attempt_retry(Ok(pb::AttemptRetryResponse {
            attempt_token: "tok-2".to_string(),
            ..Default::default()
        }));

        let input = make_function_input();
        let item = pb::FunctionPutInputsItem {
            idx: 0,
            input: Some(input),
            ..Default::default()
        };
        let mut inv = InputPlaneInvocation {
            function_id: "fn-2".to_string(),
            input: item,
            attempt_token: "tok-1".to_string(),
        };

        inv.retry(&mock, 0).unwrap();
        assert_eq!(inv.attempt_token, "tok-2");
    }

    // === process_result Tests ===

    #[test]
    fn test_process_result_success_cbor() {
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;
        let cbor_data = make_cbor_bytes(&ciborium::Value::Float(3.14));

        let result = process_result(
            &mock,
            &downloader,
            Some(&pb::GenericResult {
                status: pb::generic_result::GenericStatus::Success as i32,
                data_oneof: Some(pb::generic_result::DataOneof::Data(cbor_data)),
                ..Default::default()
            }),
            pb::DataFormat::Cbor as i32,
        )
        .unwrap();

        match result {
            InvocationResult::Cbor(ciborium::Value::Float(f)) => {
                assert!((f - 3.14).abs() < 0.001);
            }
            other => panic!("expected Cbor float, got: {:?}", other),
        }
    }

    #[test]
    fn test_process_result_success_with_blob() {
        let mock = MockInvocationClient::new();
        mock.push_blob_get(Ok(pb::BlobGetResponse {
            download_url: "https://example.com/blob".to_string(),
            ..Default::default()
        }));

        let cbor_data = make_cbor_bytes(&ciborium::Value::Integer(99.into()));
        let downloader = MockBlobDownloader::new();
        downloader.push(Ok(cbor_data));

        let result = process_result(
            &mock,
            &downloader,
            Some(&pb::GenericResult {
                status: pb::generic_result::GenericStatus::Success as i32,
                data_oneof: Some(pb::generic_result::DataOneof::DataBlobId(
                    "blob-123".to_string(),
                )),
                ..Default::default()
            }),
            pb::DataFormat::Cbor as i32,
        )
        .unwrap();

        assert_eq!(result, InvocationResult::Cbor(ciborium::Value::Integer(99.into())));
    }

    #[test]
    fn test_process_result_null_result() {
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;

        let err = process_result(&mock, &downloader, None, pb::DataFormat::Cbor as i32).unwrap_err();

        match err {
            ModalError::Remote(msg) => assert!(msg.contains("null result")),
            other => panic!("expected Remote, got: {:?}", other),
        }
    }

    #[test]
    fn test_process_result_timeout_status() {
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;

        let err = process_result(
            &mock,
            &downloader,
            Some(&pb::GenericResult {
                status: pb::generic_result::GenericStatus::Timeout as i32,
                exception: "timed out".to_string(),
                ..Default::default()
            }),
            pb::DataFormat::Cbor as i32,
        )
        .unwrap_err();

        match err {
            ModalError::FunctionTimeout(msg) => assert_eq!(msg, "timed out"),
            other => panic!("expected FunctionTimeout, got: {:?}", other),
        }
    }

    #[test]
    fn test_process_result_internal_failure_status() {
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;

        let err = process_result(
            &mock,
            &downloader,
            Some(&pb::GenericResult {
                status: pb::generic_result::GenericStatus::InternalFailure as i32,
                exception: "internal boom".to_string(),
                ..Default::default()
            }),
            pb::DataFormat::Cbor as i32,
        )
        .unwrap_err();

        match err {
            ModalError::InternalFailure(msg) => assert_eq!(msg, "internal boom"),
            other => panic!("expected InternalFailure, got: {:?}", other),
        }
    }

    #[test]
    fn test_process_result_failure_status() {
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;

        let err = process_result(
            &mock,
            &downloader,
            Some(&pb::GenericResult {
                status: pb::generic_result::GenericStatus::Failure as i32,
                exception: "user error".to_string(),
                ..Default::default()
            }),
            pb::DataFormat::Cbor as i32,
        )
        .unwrap_err();

        match err {
            ModalError::Remote(msg) => assert_eq!(msg, "user error"),
            other => panic!("expected Remote, got: {:?}", other),
        }
    }

    #[test]
    fn test_process_result_terminated_status() {
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;

        let err = process_result(
            &mock,
            &downloader,
            Some(&pb::GenericResult {
                status: pb::generic_result::GenericStatus::Terminated as i32,
                exception: "killed".to_string(),
                ..Default::default()
            }),
            pb::DataFormat::Cbor as i32,
        )
        .unwrap_err();

        match err {
            ModalError::Remote(msg) => assert_eq!(msg, "killed"),
            other => panic!("expected Remote, got: {:?}", other),
        }
    }

    // === deserialize_data_format Tests ===

    #[test]
    fn test_deserialize_cbor_integer() {
        let data = make_cbor_bytes(&ciborium::Value::Integer(42.into()));
        let result = deserialize_data_format(Some(&data), pb::DataFormat::Cbor as i32).unwrap();
        assert_eq!(result, InvocationResult::Cbor(ciborium::Value::Integer(42.into())));
    }

    #[test]
    fn test_deserialize_cbor_string() {
        let data = make_cbor_bytes(&ciborium::Value::Text("hello world".to_string()));
        let result = deserialize_data_format(Some(&data), pb::DataFormat::Cbor as i32).unwrap();
        assert_eq!(
            result,
            InvocationResult::Cbor(ciborium::Value::Text("hello world".to_string()))
        );
    }

    #[test]
    fn test_deserialize_cbor_null_data() {
        let result = deserialize_data_format(None, pb::DataFormat::Cbor as i32).unwrap();
        assert_eq!(result, InvocationResult::Null);
    }

    #[test]
    fn test_deserialize_cbor_empty_data() {
        let result = deserialize_data_format(Some(&[]), pb::DataFormat::Cbor as i32).unwrap();
        assert_eq!(result, InvocationResult::Null);
    }

    #[test]
    fn test_deserialize_pickle_unsupported() {
        let err =
            deserialize_data_format(Some(&[1, 2, 3]), pb::DataFormat::Pickle as i32).unwrap_err();
        match err {
            ModalError::Serialization(msg) => assert!(msg.contains("PICKLE")),
            other => panic!("expected Serialization, got: {:?}", other),
        }
    }

    #[test]
    fn test_deserialize_asgi_unsupported() {
        let err =
            deserialize_data_format(Some(&[1, 2, 3]), pb::DataFormat::Asgi as i32).unwrap_err();
        match err {
            ModalError::Serialization(msg) => assert!(msg.contains("ASGI")),
            other => panic!("expected Serialization, got: {:?}", other),
        }
    }

    #[test]
    fn test_deserialize_generator_done() {
        let data = vec![1, 2, 3];
        let result =
            deserialize_data_format(Some(&data), pb::DataFormat::GeneratorDone as i32).unwrap();
        assert_eq!(result, InvocationResult::GeneratorDone(vec![1, 2, 3]));
    }

    #[test]
    fn test_deserialize_generator_done_null() {
        let result =
            deserialize_data_format(None, pb::DataFormat::GeneratorDone as i32).unwrap();
        assert_eq!(result, InvocationResult::GeneratorDone(Vec::new()));
    }

    #[test]
    fn test_deserialize_unsupported_format() {
        let err = deserialize_data_format(Some(&[1]), 999).unwrap_err();
        match err {
            ModalError::Serialization(msg) => assert!(msg.contains("unsupported")),
            other => panic!("expected Serialization, got: {:?}", other),
        }
    }

    // === CBOR serialization helpers ===

    #[test]
    fn test_cbor_serialize_roundtrip() {
        let args = vec![ciborium::Value::Integer(1.into()), ciborium::Value::Text("hello".to_string())];
        let kwargs = ciborium::Value::Map(vec![
            (
                ciborium::Value::Text("key".to_string()),
                ciborium::Value::Bool(true),
            ),
        ]);

        let data = cbor_serialize(&args, &kwargs).unwrap();
        let deserialized = cbor_deserialize(&data).unwrap();

        match deserialized {
            ciborium::Value::Array(items) => {
                assert_eq!(items.len(), 2);
                // First element is the args array
                match &items[0] {
                    ciborium::Value::Array(a) => assert_eq!(a.len(), 2),
                    other => panic!("expected array, got: {:?}", other),
                }
                // Second element is the kwargs map
                match &items[1] {
                    ciborium::Value::Map(m) => assert_eq!(m.len(), 1),
                    other => panic!("expected map, got: {:?}", other),
                }
            }
            other => panic!("expected array, got: {:?}", other),
        }
    }

    #[test]
    fn test_cbor_serialize_empty() {
        let args = vec![];
        let kwargs = ciborium::Value::Map(vec![]);

        let data = cbor_serialize(&args, &kwargs).unwrap();
        let deserialized = cbor_deserialize(&data).unwrap();

        match deserialized {
            ciborium::Value::Array(items) => {
                assert_eq!(items.len(), 2);
                assert_eq!(items[0], ciborium::Value::Array(vec![]));
                assert_eq!(items[1], ciborium::Value::Map(vec![]));
            }
            other => panic!("expected array, got: {:?}", other),
        }
    }

    #[test]
    fn test_cbor_deserialize_invalid() {
        let err = cbor_deserialize(&[0xFF, 0xFE]).unwrap_err();
        match err {
            ModalError::Serialization(msg) => assert!(msg.contains("CBOR")),
            other => panic!("expected Serialization, got: {:?}", other),
        }
    }

    // === Constants ===

    #[test]
    fn test_max_system_retries() {
        assert_eq!(max_system_retries(), 8);
    }

    #[test]
    fn test_max_object_size_bytes() {
        assert_eq!(max_object_size_bytes(), 2 * 1024 * 1024);
    }

    #[test]
    fn test_outputs_timeout_value() {
        assert_eq!(OUTPUTS_TIMEOUT, Duration::from_secs(55));
    }

    // === NoBlobDownloader ===

    #[test]
    fn test_no_blob_downloader() {
        let downloader = NoBlobDownloader;
        let err = downloader.download("https://example.com").unwrap_err();
        assert!(err.to_string().contains("not supported"));
    }

    // === Control plane retry without input ===
    #[test]
    fn test_control_plane_retry_no_input_errors() {
        let mock = MockInvocationClient::new();
        let mut inv = ControlPlaneInvocation::from_function_call_id("fc-1".to_string());
        let err = inv.retry(&mock, 0).unwrap_err();
        assert!(err.to_string().contains("input missing"));
    }

    // === Blob download tests ===
    #[test]
    fn test_blob_download_success() {
        let mock = MockInvocationClient::new();
        mock.push_blob_get(Ok(pb::BlobGetResponse {
            download_url: "https://s3.example.com/blob".to_string(),
            ..Default::default()
        }));

        let downloader = MockBlobDownloader::new();
        downloader.push(Ok(vec![1, 2, 3, 4]));

        let data = blob_download(&mock, &downloader, "blob-abc").unwrap();
        assert_eq!(data, vec![1, 2, 3, 4]);
    }

    #[test]
    fn test_blob_download_grpc_error() {
        let mock = MockInvocationClient::new();
        mock.push_blob_get(Err(ModalError::Other("blob not found".to_string())));

        let downloader = NoBlobDownloader;
        let err = blob_download(&mock, &downloader, "blob-missing").unwrap_err();
        assert!(err.to_string().contains("blob not found"));
    }

    #[test]
    fn test_blob_download_http_error() {
        let mock = MockInvocationClient::new();
        mock.push_blob_get(Ok(pb::BlobGetResponse {
            download_url: "https://s3.example.com/blob".to_string(),
            ..Default::default()
        }));

        let downloader = MockBlobDownloader::new();
        downloader.push(Err(ModalError::Other("download failed".to_string())));

        let err = blob_download(&mock, &downloader, "blob-abc").unwrap_err();
        assert!(err.to_string().contains("download failed"));
    }

    // === Complex CBOR types ===
    #[test]
    fn test_deserialize_cbor_array() {
        let val = ciborium::Value::Array(vec![
            ciborium::Value::Integer(1.into()),
            ciborium::Value::Integer(2.into()),
            ciborium::Value::Integer(3.into()),
        ]);
        let data = make_cbor_bytes(&val);
        let result = deserialize_data_format(Some(&data), pb::DataFormat::Cbor as i32).unwrap();
        assert_eq!(result, InvocationResult::Cbor(val));
    }

    #[test]
    fn test_deserialize_cbor_map() {
        let val = ciborium::Value::Map(vec![
            (
                ciborium::Value::Text("name".to_string()),
                ciborium::Value::Text("test".to_string()),
            ),
            (
                ciborium::Value::Text("count".to_string()),
                ciborium::Value::Integer(5.into()),
            ),
        ]);
        let data = make_cbor_bytes(&val);
        let result = deserialize_data_format(Some(&data), pb::DataFormat::Cbor as i32).unwrap();
        assert_eq!(result, InvocationResult::Cbor(val));
    }

    #[test]
    fn test_deserialize_cbor_nested() {
        let val = ciborium::Value::Array(vec![
            ciborium::Value::Map(vec![(
                ciborium::Value::Text("key".to_string()),
                ciborium::Value::Array(vec![ciborium::Value::Bool(true)]),
            )]),
        ]);
        let data = make_cbor_bytes(&val);
        let result = deserialize_data_format(Some(&data), pb::DataFormat::Cbor as i32).unwrap();
        assert_eq!(result, InvocationResult::Cbor(val));
    }

    #[test]
    fn test_time_now_seconds() {
        let t = time_now_seconds();
        // Should be a reasonable Unix timestamp (after 2020)
        assert!(t > 1_577_836_800.0);
    }

    // === Success with no data oneof ===
    #[test]
    fn test_process_result_success_no_data() {
        let mock = MockInvocationClient::new();
        let downloader = NoBlobDownloader;

        let result = process_result(
            &mock,
            &downloader,
            Some(&pb::GenericResult {
                status: pb::generic_result::GenericStatus::Success as i32,
                data_oneof: None,
                ..Default::default()
            }),
            pb::DataFormat::Cbor as i32,
        )
        .unwrap();

        assert_eq!(result, InvocationResult::Null);
    }
}
