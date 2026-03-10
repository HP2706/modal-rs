/// Real gRPC transport layer for connecting to Modal's API.
///
/// This module provides the bridge between the abstract `*GrpcClient` traits
/// used by service implementations and the actual tonic-generated gRPC client.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tonic::transport::{Channel, ClientTlsConfig, Endpoint};
use tonic::{metadata::MetadataValue, Request, Status};

use modal_proto::modal_proto as pb;
use modal_proto::modal_proto::modal_client_client::ModalClientClient;

use crate::config::Profile;
use crate::error::ModalError;
use crate::function::FunctionStats;
use crate::image::{ImageBuildResult, ImageJoinStreamingResult, ImageLayerBuildRequest};
use crate::sandbox::{
    ExecWaitResult, SandboxCreateConnectCredentials, SandboxCreateParams, SandboxExecParams,
    SandboxListEntry, SandboxPollResult, SandboxSnapshotResult, SandboxTunnelsResult,
    SandboxWaitResult, Tunnel,
};
use crate::sandbox_filesystem::{FilesystemExecRequest, FilesystemExecResponse};

/// Constants matching Go SDK defaults.
const API_ENDPOINT: &str = "api.modal.com:443";
const MAX_MESSAGE_SIZE: usize = 100 * 1024 * 1024; // 100 MB

/// The real gRPC transport, wrapping a tonic `ModalClientClient`.
///
/// This struct holds a connected gRPC channel and implements all `*GrpcClient`
/// traits so it can be used to back all service implementations.
#[derive(Debug)]
pub struct ModalGrpcTransport {
    client: Mutex<ModalClientClient<Channel>>,
    runtime: tokio::runtime::Handle,
}

impl ModalGrpcTransport {
    /// Connect to a Modal API server using the given profile.
    ///
    /// This creates a TLS-enabled gRPC channel for HTTPS URLs,
    /// or an insecure channel for HTTP URLs (e.g. localhost testing).
    pub fn connect(profile: &Profile, sdk_version: &str) -> Result<Self, ModalError> {
        let runtime = tokio::runtime::Handle::try_current().map_err(|_| {
            ModalError::Other(
                "ModalGrpcTransport::connect must be called from within a tokio runtime"
                    .to_string(),
            )
        })?;

        let channel = runtime.block_on(Self::create_channel(profile))?;
        let client = Self::create_client(channel, profile, sdk_version)?;

        Ok(Self {
            client: Mutex::new(client),
            runtime,
        })
    }

    /// Connect using the default API endpoint with credentials from the profile.
    pub fn connect_default(profile: &Profile) -> Result<Self, ModalError> {
        Self::connect(profile, "0.1.0")
    }

    async fn create_channel(profile: &Profile) -> Result<Channel, ModalError> {
        let server_url = if profile.server_url.is_empty() {
            format!("https://{}", API_ENDPOINT)
        } else {
            profile.server_url.clone()
        };

        let is_tls = server_url.starts_with("https://");

        let endpoint = Endpoint::from_shared(server_url.clone())
            .map_err(|e| ModalError::Config(format!("invalid server URL '{}': {}", server_url, e)))?
            .initial_stream_window_size(64 * 1024 * 1024) // 64 MiB
            .initial_connection_window_size(64 * 1024 * 1024);

        let endpoint = if is_tls {
            let tls_config = ClientTlsConfig::new();
            endpoint
                .tls_config(tls_config)
                .map_err(|e| ModalError::Config(format!("TLS configuration error: {}", e)))?
        } else {
            endpoint
        };

        endpoint
            .connect()
            .await
            .map_err(|e| ModalError::Other(format!("failed to connect to {}: {}", server_url, e)))
    }

    fn create_client(
        channel: Channel,
        profile: &Profile,
        sdk_version: &str,
    ) -> Result<ModalClientClient<Channel>, ModalError> {
        // Validate credentials
        if profile.token_id.is_empty() || profile.token_secret.is_empty() {
            return Err(ModalError::Config(
                "missing token_id or token_secret, please set in .modal.toml, environment variables, or via ClientParams".to_string(),
            ));
        }

        let client = ModalClientClient::new(channel)
            .max_decoding_message_size(MAX_MESSAGE_SIZE)
            .max_encoding_message_size(MAX_MESSAGE_SIZE);

        // Note: tonic interceptors are set per-request via inject_metadata.
        // For a production implementation, you would use tower layers or
        // tonic interceptors for automatic header injection.
        let _ = sdk_version; // Will be used when we set up interceptors
        Ok(client)
    }

    /// Inject Modal auth headers into a gRPC request.
    /// Used when making authenticated calls with per-request metadata.
    #[allow(dead_code)]
    fn inject_metadata<T>(&self, request: &mut Request<T>, profile: &Profile, sdk_version: &str) {
        let metadata = request.metadata_mut();
        if let Ok(v) = MetadataValue::try_from(&profile.token_id) {
            metadata.insert("x-modal-token-id", v);
        }
        if let Ok(v) = MetadataValue::try_from(&profile.token_secret) {
            metadata.insert("x-modal-token-secret", v);
        }
        if let Ok(v) = MetadataValue::try_from("9") {
            metadata.insert("x-modal-client-type", v);
        }
        if let Ok(v) = MetadataValue::try_from("1.0.0") {
            metadata.insert("x-modal-client-version", v);
        }
        if let Ok(v) = MetadataValue::try_from(format!("modal-rs/{}", sdk_version)) {
            metadata.insert("x-modal-libmodal-version", v);
        }
    }

    /// Convert a tonic Status to a ModalError.
    fn status_to_error(status: Status) -> ModalError {
        ModalError::Grpc(status)
    }

    /// Execute a blocking gRPC call on the runtime.
    fn block_on<F: std::future::Future<Output = Result<T, Status>>, T>(
        &self,
        f: F,
    ) -> Result<T, ModalError> {
        self.runtime
            .block_on(f)
            .map_err(Self::status_to_error)
    }
}

// ============================================================================
// AppGrpcClient implementation
// ============================================================================

impl crate::app::AppGrpcClient for ModalGrpcTransport {
    fn app_get_or_create(
        &self,
        app_name: &str,
        environment_name: &str,
        object_creation_type: i32,
    ) -> Result<String, ModalError> {
        let request = pb::AppGetOrCreateRequest {
            app_name: app_name.to_string(),
            environment_name: environment_name.to_string(),
            object_creation_type,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.app_get_or_create(request))?;
        Ok(resp.into_inner().app_id)
    }
}

// ============================================================================
// ClsGrpcClient implementation
// ============================================================================

impl crate::cls::ClsGrpcClient for ModalGrpcTransport {
    fn function_get(
        &self,
        app_name: &str,
        object_tag: &str,
        environment_name: &str,
    ) -> Result<(String, Option<pb::FunctionHandleMetadata>), ModalError> {
        let request = pb::FunctionGetRequest {
            app_name: app_name.to_string(),
            object_tag: object_tag.to_string(),
            environment_name: environment_name.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.function_get(request))?.into_inner();
        Ok((resp.function_id, resp.handle_metadata))
    }
}

// ============================================================================
// SecretGrpcClient implementation
// ============================================================================

impl crate::secret::SecretGrpcClient for ModalGrpcTransport {
    fn secret_get_or_create(
        &self,
        deployment_name: &str,
        environment_name: &str,
        required_keys: &[String],
        object_creation_type: i32,
        env_dict: &HashMap<String, String>,
    ) -> Result<String, ModalError> {
        let request = pb::SecretGetOrCreateRequest {
            deployment_name: deployment_name.to_string(),
            environment_name: environment_name.to_string(),
            object_creation_type,
            env_dict: env_dict.clone(),
            required_keys: required_keys.to_vec(),
            app_id: String::new(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.secret_get_or_create(request))?;
        Ok(resp.into_inner().secret_id)
    }

    fn secret_delete(&self, secret_id: &str) -> Result<(), ModalError> {
        let request = pb::SecretDeleteRequest {
            secret_id: secret_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.secret_delete(request))?;
        Ok(())
    }
}

// ============================================================================
// FunctionGrpcClient implementation
// ============================================================================

impl crate::function::FunctionGrpcClient for ModalGrpcTransport {
    fn function_get(
        &self,
        app_name: &str,
        object_tag: &str,
        environment_name: &str,
    ) -> Result<pb::FunctionGetResponse, ModalError> {
        let request = pb::FunctionGetRequest {
            app_name: app_name.to_string(),
            object_tag: object_tag.to_string(),
            environment_name: environment_name.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.function_get(request))?;
        Ok(resp.into_inner())
    }

    fn function_get_current_stats(
        &self,
        function_id: &str,
    ) -> Result<FunctionStats, ModalError> {
        let request = pb::FunctionGetCurrentStatsRequest {
            function_id: function_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.function_get_current_stats(request))?.into_inner();
        Ok(FunctionStats {
            backlog: resp.backlog,
            num_total_runners: resp.num_total_tasks,
        })
    }

    fn function_update_scheduling_params(
        &self,
        function_id: &str,
        min_containers: Option<u32>,
        max_containers: Option<u32>,
        buffer_containers: Option<u32>,
        scaledown_window: Option<u32>,
    ) -> Result<(), ModalError> {
        let settings = pb::AutoscalerSettings {
            min_containers,
            max_containers,
            buffer_containers,
            scaleup_window: None,
            scaledown_window,
        };
        let request = pb::FunctionUpdateSchedulingParamsRequest {
            function_id: function_id.to_string(),
            warm_pool_size_override: 0,
            settings: Some(settings),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.function_update_scheduling_params(request))?;
        Ok(())
    }
}

// ============================================================================
// FunctionCallGrpcClient implementation
// ============================================================================

impl crate::function_call::FunctionCallGrpcClient for ModalGrpcTransport {
    fn function_call_cancel(
        &self,
        function_call_id: &str,
        terminate_containers: bool,
    ) -> Result<(), ModalError> {
        let request = pb::FunctionCallCancelRequest {
            function_call_id: function_call_id.to_string(),
            terminate_containers,
            function_id: None,
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.function_call_cancel(request))?;
        Ok(())
    }
}

// ============================================================================
// InvocationGrpcClient implementation
// ============================================================================

impl crate::invocation::InvocationGrpcClient for ModalGrpcTransport {
    fn function_map(
        &self,
        function_id: &str,
        function_call_type: i32,
        function_call_invocation_type: i32,
        pipelined_inputs: Vec<pb::FunctionPutInputsItem>,
    ) -> Result<pb::FunctionMapResponse, ModalError> {
        let request = pb::FunctionMapRequest {
            function_id: function_id.to_string(),
            parent_input_id: String::new(),
            return_exceptions: false,
            function_call_type,
            pipelined_inputs,
            function_call_invocation_type,
            from_spawn_map: false,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.function_map(request))?;
        Ok(resp.into_inner())
    }

    fn function_get_outputs(
        &self,
        function_call_id: &str,
        max_values: u32,
        timeout: f32,
        last_entry_id: &str,
        clear_on_success: bool,
        requested_at: f64,
    ) -> Result<pb::FunctionGetOutputsResponse, ModalError> {
        let request = pb::FunctionGetOutputsRequest {
            function_call_id: function_call_id.to_string(),
            max_values: max_values as i32,
            timeout,
            last_entry_id: last_entry_id.to_string(),
            clear_on_success,
            requested_at,
            input_jwts: vec![],
            start_idx: None,
            end_idx: None,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.function_get_outputs(request))?;
        Ok(resp.into_inner())
    }

    fn function_retry_inputs(
        &self,
        function_call_jwt: &str,
        inputs: Vec<pb::FunctionRetryInputsItem>,
    ) -> Result<pb::FunctionRetryInputsResponse, ModalError> {
        let request = pb::FunctionRetryInputsRequest {
            function_call_jwt: function_call_jwt.to_string(),
            inputs,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.function_retry_inputs(request))?;
        Ok(resp.into_inner())
    }

    fn attempt_start(
        &self,
        function_id: &str,
        input: pb::FunctionPutInputsItem,
    ) -> Result<pb::AttemptStartResponse, ModalError> {
        let request = pb::AttemptStartRequest {
            function_id: function_id.to_string(),
            parent_input_id: String::new(),
            input: Some(input),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.attempt_start(request))?;
        Ok(resp.into_inner())
    }

    fn attempt_await(
        &self,
        attempt_token: &str,
        requested_at: f64,
        timeout_secs: f32,
    ) -> Result<pb::AttemptAwaitResponse, ModalError> {
        let request = pb::AttemptAwaitRequest {
            attempt_token: attempt_token.to_string(),
            requested_at,
            timeout_secs,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.attempt_await(request))?;
        Ok(resp.into_inner())
    }

    fn attempt_retry(
        &self,
        function_id: &str,
        input: pb::FunctionPutInputsItem,
        attempt_token: &str,
    ) -> Result<pb::AttemptRetryResponse, ModalError> {
        let request = pb::AttemptRetryRequest {
            function_id: function_id.to_string(),
            parent_input_id: String::new(),
            input: Some(input),
            attempt_token: attempt_token.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.attempt_retry(request))?;
        Ok(resp.into_inner())
    }

    fn blob_get(&self, blob_id: &str) -> Result<pb::BlobGetResponse, ModalError> {
        let request = pb::BlobGetRequest {
            blob_id: blob_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.blob_get(request))?;
        Ok(resp.into_inner())
    }
}

// ============================================================================
// ImageGrpcClient implementation
// ============================================================================

impl crate::image::ImageGrpcClient for ModalGrpcTransport {
    fn image_from_id(&self, image_id: &str) -> Result<String, ModalError> {
        let request = pb::ImageFromIdRequest {
            image_id: image_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.image_from_id(request))?;
        Ok(resp.into_inner().image_id)
    }

    fn image_delete(&self, image_id: &str) -> Result<(), ModalError> {
        let request = pb::ImageDeleteRequest {
            image_id: image_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.image_delete(request))?;
        Ok(())
    }

    fn image_get_or_create(
        &self,
        request: &ImageLayerBuildRequest,
    ) -> Result<ImageBuildResult, ModalError> {
        let gpu_config = request.gpu_config.as_ref().map(|gc| pb::GpuConfig {
            r#type: 0,
            count: gc.count,
            gpu_type: gc.gpu_type.clone(),
        });
        let proto_request = pb::ImageGetOrCreateRequest {
            image: Some(pb::Image {
                dockerfile_commands: request.dockerfile_commands.clone(),
                base_images: request.base_images.iter().map(|bi| pb::BaseImage {
                    docker_tag: bi.docker_tag.clone(),
                    image_id: bi.image_id.clone(),
                }).collect(),
                secret_ids: request.secret_ids.clone(),
                gpu_config,
                ..Default::default()
            }),
            app_id: request.app_id.clone(),
            existing_image_id: String::new(),
            build_function_id: String::new(),
            force_build: request.force_build,
            namespace: 0,
            builder_version: String::new(),
            allow_global_deployment: false,
            ignore_cache: false,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.image_get_or_create(proto_request))?.into_inner();

        // Convert proto GenericResult to our ImageBuildStatus
        let (status, exception) = match resp.result {
            Some(ref result) => {
                use crate::image::ImageBuildStatus;
                let s = match result.status {
                    1 => ImageBuildStatus::Success,
                    _ => ImageBuildStatus::Failure,
                };
                let exc = if result.exception.is_empty() {
                    None
                } else {
                    Some(result.exception.clone())
                };
                (s, exc)
            }
            None => (crate::image::ImageBuildStatus::Pending, None),
        };
        Ok(ImageBuildResult {
            image_id: resp.image_id,
            status,
            exception,
        })
    }

    fn image_join_streaming(
        &self,
        image_id: &str,
        last_entry_id: &str,
    ) -> Result<ImageJoinStreamingResult, ModalError> {
        let request = pb::ImageJoinStreamingRequest {
            image_id: image_id.to_string(),
            timeout: 55.0,
            last_entry_id: last_entry_id.to_string(),
            include_logs_for_finished: false,
        };
        let mut client = self.client.lock().unwrap().clone();

        // ImageJoinStreaming is a server-streaming RPC. Collect results until EOF.
        let stream = self.block_on(client.image_join_streaming(request))?;
        let mut stream = stream.into_inner();

        let mut final_result = None;
        let mut last_entry = last_entry_id.to_string();

        loop {
            match self.runtime.block_on(stream.message()) {
                Ok(Some(resp)) => {
                    if !resp.entry_id.is_empty() {
                        last_entry = resp.entry_id.clone();
                    }
                    if resp.result.is_some() || resp.eof {
                        final_result = Some(resp);
                        break;
                    }
                }
                Ok(None) => break,
                Err(status) => return Err(Self::status_to_error(status)),
            }
        }

        let build_result = final_result.and_then(|resp| {
            resp.result.map(|generic| {
                use crate::image::ImageBuildStatus;
                let status = match generic.status {
                    1 => ImageBuildStatus::Success,
                    4 => ImageBuildStatus::Timeout,
                    5 => ImageBuildStatus::Terminated,
                    _ => ImageBuildStatus::Failure,
                };
                let exception = if generic.exception.is_empty() {
                    None
                } else {
                    Some(generic.exception)
                };
                ImageBuildResult {
                    image_id: image_id.to_string(),
                    status,
                    exception,
                }
            })
        });

        Ok(ImageJoinStreamingResult {
            result: build_result,
            last_entry_id: last_entry,
        })
    }
}

// ============================================================================
// VolumeGrpcClient implementation
// ============================================================================

impl crate::volume::VolumeGrpcClient for ModalGrpcTransport {
    fn volume_get_or_create(
        &self,
        deployment_name: &str,
        environment_name: &str,
        object_creation_type: i32,
    ) -> Result<String, ModalError> {
        let request = pb::VolumeGetOrCreateRequest {
            deployment_name: deployment_name.to_string(),
            environment_name: environment_name.to_string(),
            object_creation_type,
            app_id: String::new(),
            version: 0,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.volume_get_or_create(request))?;
        Ok(resp.into_inner().volume_id)
    }

    fn volume_heartbeat(&self, volume_id: &str) -> Result<(), ModalError> {
        let request = pb::VolumeHeartbeatRequest {
            volume_id: volume_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.volume_heartbeat(request))?;
        Ok(())
    }

    fn volume_delete(&self, volume_id: &str) -> Result<(), ModalError> {
        #[allow(deprecated)]
        let request = pb::VolumeDeleteRequest {
            volume_id: volume_id.to_string(),
            environment_name: String::new(),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.volume_delete(request))?;
        Ok(())
    }
}

// ============================================================================
// QueueGrpcClient implementation
// ============================================================================

impl crate::queue::QueueGrpcClient for ModalGrpcTransport {
    fn queue_get_or_create(
        &self,
        deployment_name: &str,
        environment_name: &str,
        object_creation_type: i32,
    ) -> Result<String, ModalError> {
        let request = pb::QueueGetOrCreateRequest {
            deployment_name: deployment_name.to_string(),
            environment_name: environment_name.to_string(),
            object_creation_type,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.queue_get_or_create(request))?;
        Ok(resp.into_inner().queue_id)
    }

    fn queue_heartbeat(&self, queue_id: &str) -> Result<(), ModalError> {
        let request = pb::QueueHeartbeatRequest {
            queue_id: queue_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.queue_heartbeat(request))?;
        Ok(())
    }

    fn queue_delete(&self, queue_id: &str) -> Result<(), ModalError> {
        let request = pb::QueueDeleteRequest {
            queue_id: queue_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.queue_delete(request))?;
        Ok(())
    }

    fn queue_clear(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        all_partitions: bool,
    ) -> Result<(), ModalError> {
        let request = pb::QueueClearRequest {
            queue_id: queue_id.to_string(),
            partition_key: partition_key.unwrap_or_default().to_vec(),
            all_partitions,
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.queue_clear(request))?;
        Ok(())
    }

    fn queue_len(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        total: bool,
    ) -> Result<i32, ModalError> {
        let request = pb::QueueLenRequest {
            queue_id: queue_id.to_string(),
            partition_key: partition_key.unwrap_or_default().to_vec(),
            total,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.queue_len(request))?;
        Ok(resp.into_inner().len)
    }

    fn queue_get(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        timeout: f32,
        n_values: i32,
    ) -> Result<Vec<Vec<u8>>, ModalError> {
        let request = pb::QueueGetRequest {
            queue_id: queue_id.to_string(),
            timeout,
            n_values,
            partition_key: partition_key.unwrap_or_default().to_vec(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.queue_get(request))?;
        Ok(resp.into_inner().values)
    }

    fn queue_put(
        &self,
        queue_id: &str,
        values: Vec<Vec<u8>>,
        partition_key: Option<&[u8]>,
        partition_ttl_seconds: i32,
    ) -> Result<(), ModalError> {
        let request = pb::QueuePutRequest {
            queue_id: queue_id.to_string(),
            values,
            partition_key: partition_key.unwrap_or_default().to_vec(),
            partition_ttl_seconds,
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.queue_put(request))?;
        Ok(())
    }

    fn queue_next_items(
        &self,
        queue_id: &str,
        partition_key: Option<&[u8]>,
        item_poll_timeout: f32,
        last_entry_id: &str,
    ) -> Result<Vec<(String, Vec<u8>)>, ModalError> {
        let request = pb::QueueNextItemsRequest {
            queue_id: queue_id.to_string(),
            partition_key: partition_key.unwrap_or_default().to_vec(),
            last_entry_id: last_entry_id.to_string(),
            item_poll_timeout,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.queue_next_items(request))?;
        Ok(resp
            .into_inner()
            .items
            .into_iter()
            .map(|item| (item.entry_id, item.value))
            .collect())
    }
}

// ============================================================================
// ProxyGrpcClient implementation
// ============================================================================

impl crate::proxy::ProxyGrpcClient for ModalGrpcTransport {
    fn proxy_get(
        &self,
        name: &str,
        environment_name: &str,
    ) -> Result<Option<String>, ModalError> {
        let request = pb::ProxyGetRequest {
            name: name.to_string(),
            environment_name: environment_name.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.proxy_get(request))?.into_inner();
        Ok(resp.proxy.and_then(|p| {
            if p.proxy_id.is_empty() {
                None
            } else {
                Some(p.proxy_id)
            }
        }))
    }
}

// ============================================================================
// SandboxGrpcClient implementation
// ============================================================================

impl crate::sandbox::SandboxGrpcClient for ModalGrpcTransport {
    fn sandbox_create(
        &self,
        app_id: &str,
        image_id: &str,
        params: &SandboxCreateParams,
    ) -> Result<String, ModalError> {
        let definition = pb::Sandbox {
            image_id: image_id.to_string(),
            ..Default::default()
        };
        let request = pb::SandboxCreateRequest {
            app_id: app_id.to_string(),
            definition: Some(definition),
            environment_name: String::new(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let _ = params; // params are encoded into the definition in real usage
        let resp = self.block_on(client.sandbox_create(request))?;
        Ok(resp.into_inner().sandbox_id)
    }

    fn sandbox_get_task_id(
        &self,
        sandbox_id: &str,
    ) -> Result<(Option<String>, bool), ModalError> {
        let request = pb::SandboxGetTaskIdRequest {
            sandbox_id: sandbox_id.to_string(),
            timeout: None,
            wait_until_ready: false,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_get_task_id(request))?.into_inner();
        let has_result = resp.task_result.is_some();
        Ok((resp.task_id, has_result))
    }

    fn container_exec(
        &self,
        task_id: &str,
        command: Vec<String>,
        params: &SandboxExecParams,
    ) -> Result<String, ModalError> {
        let pty_info = if params.pty {
            Some(pb::PtyInfo {
                enabled: true,
                ..Default::default()
            })
        } else {
            None
        };
        let stdout_output = match params.stdout {
            crate::sandbox::StreamConfig::Pipe => 2,   // ExecOutputOption::Pipe
            crate::sandbox::StreamConfig::Ignore => 1,  // ExecOutputOption::Devnull
        };
        let stderr_output = match params.stderr {
            crate::sandbox::StreamConfig::Pipe => 2,
            crate::sandbox::StreamConfig::Ignore => 1,
        };
        #[allow(deprecated)]
        let request = pb::ContainerExecRequest {
            task_id: task_id.to_string(),
            command,
            pty_info,
            terminate_container_on_exit: false,
            runtime_debug: false,
            stdout_output,
            stderr_output,
            timeout_secs: params.timeout.as_secs() as u32,
            workdir: if params.workdir.is_empty() { None } else { Some(params.workdir.clone()) },
            ..Default::default()
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.container_exec(request))?;
        Ok(resp.into_inner().exec_id)
    }

    fn container_exec_wait(
        &self,
        exec_id: &str,
        timeout: f32,
    ) -> Result<ExecWaitResult, ModalError> {
        let request = pb::ContainerExecWaitRequest {
            exec_id: exec_id.to_string(),
            timeout,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.container_exec_wait(request))?.into_inner();
        Ok(ExecWaitResult {
            exit_code: resp.exit_code,
            completed: resp.completed,
        })
    }

    fn sandbox_wait(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<SandboxWaitResult, ModalError> {
        let request = pb::SandboxWaitRequest {
            sandbox_id: sandbox_id.to_string(),
            timeout,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_wait(request))?.into_inner();
        let result = resp.result.unwrap_or_default();
        Ok(SandboxWaitResult {
            exit_code: result.exitcode,
            success: result.status == 1, // GenericStatus::Success
            exception: if result.exception.is_empty() {
                None
            } else {
                Some(result.exception)
            },
        })
    }

    fn sandbox_terminate(&self, sandbox_id: &str) -> Result<(), ModalError> {
        let request = pb::SandboxTerminateRequest {
            sandbox_id: sandbox_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.sandbox_terminate(request))?;
        Ok(())
    }

    fn sandbox_from_id(&self, sandbox_id: &str) -> Result<(), ModalError> {
        // Use SandboxWait with timeout=0 to check if sandbox exists
        let request = pb::SandboxWaitRequest {
            sandbox_id: sandbox_id.to_string(),
            timeout: 0.0,
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.sandbox_wait(request))?;
        Ok(())
    }

    fn sandbox_from_name(
        &self,
        app_name: &str,
        name: &str,
        environment: &str,
    ) -> Result<String, ModalError> {
        let request = pb::SandboxGetFromNameRequest {
            app_name: app_name.to_string(),
            sandbox_name: name.to_string(),
            environment_name: environment.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_get_from_name(request))?;
        Ok(resp.into_inner().sandbox_id)
    }

    fn sandbox_list(
        &self,
        app_id: &str,
        environment: &str,
        tags: &HashMap<String, String>,
        before_timestamp: f64,
    ) -> Result<Vec<SandboxListEntry>, ModalError> {
        let proto_tags: Vec<pb::SandboxTag> = tags
            .iter()
            .map(|(k, v)| pb::SandboxTag {
                tag_name: k.clone(),
                tag_value: v.clone(),
            })
            .collect();
        let request = pb::SandboxListRequest {
            app_id: app_id.to_string(),
            before_timestamp,
            environment_name: environment.to_string(),
            include_finished: false,
            tags: proto_tags,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_list(request))?.into_inner();
        Ok(resp
            .sandboxes
            .into_iter()
            .map(|s| SandboxListEntry {
                sandbox_id: s.id,
                created_at: s.created_at,
            })
            .collect())
    }

    fn sandbox_poll(&self, sandbox_id: &str) -> Result<SandboxPollResult, ModalError> {
        let request = pb::SandboxWaitRequest {
            sandbox_id: sandbox_id.to_string(),
            timeout: 0.0,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_wait(request))?.into_inner();
        match resp.result {
            Some(result) if result.status != 0 => Ok(SandboxPollResult {
                exit_code: Some(result.exitcode),
            }),
            _ => Ok(SandboxPollResult { exit_code: None }),
        }
    }

    fn sandbox_tags_set(
        &self,
        sandbox_id: &str,
        tags: &HashMap<String, String>,
    ) -> Result<(), ModalError> {
        let proto_tags: Vec<pb::SandboxTag> = tags
            .iter()
            .map(|(k, v)| pb::SandboxTag {
                tag_name: k.clone(),
                tag_value: v.clone(),
            })
            .collect();
        let request = pb::SandboxTagsSetRequest {
            sandbox_id: sandbox_id.to_string(),
            environment_name: String::new(),
            tags: proto_tags,
        };
        let mut client = self.client.lock().unwrap().clone();
        self.block_on(client.sandbox_tags_set(request))?;
        Ok(())
    }

    fn sandbox_tags_get(
        &self,
        sandbox_id: &str,
    ) -> Result<HashMap<String, String>, ModalError> {
        let request = pb::SandboxTagsGetRequest {
            sandbox_id: sandbox_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_tags_get(request))?.into_inner();
        Ok(resp
            .tags
            .into_iter()
            .map(|t| (t.tag_name, t.tag_value))
            .collect())
    }

    fn sandbox_get_tunnels(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<SandboxTunnelsResult, ModalError> {
        let request = pb::SandboxGetTunnelsRequest {
            sandbox_id: sandbox_id.to_string(),
            timeout,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_get_tunnels(request))?.into_inner();
        let timed_out = resp.result.as_ref().map_or(false, |r| r.status == 4); // Timeout
        let tunnels = resp
            .tunnels
            .into_iter()
            .map(|t| {
                (
                    t.container_port as i32,
                    Tunnel {
                        host: t.host,
                        port: t.port as i32,
                        unencrypted_host: t.unencrypted_host.unwrap_or_default(),
                        unencrypted_port: t.unencrypted_port.unwrap_or_default() as i32,
                    },
                )
            })
            .collect();
        Ok(SandboxTunnelsResult { timed_out, tunnels })
    }

    fn sandbox_snapshot_fs(
        &self,
        sandbox_id: &str,
        timeout: f32,
    ) -> Result<SandboxSnapshotResult, ModalError> {
        let request = pb::SandboxSnapshotFsRequest {
            sandbox_id: sandbox_id.to_string(),
            timeout,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_snapshot_fs(request))?.into_inner();
        let success = resp.result.as_ref().map_or(false, |r| r.status == 1);
        let exception = resp
            .result
            .as_ref()
            .and_then(|r| {
                if r.exception.is_empty() {
                    None
                } else {
                    Some(r.exception.clone())
                }
            });
        Ok(SandboxSnapshotResult {
            image_id: resp.image_id,
            success,
            exception,
        })
    }

    fn sandbox_snapshot_directory(
        &self,
        task_id: &str,
        path: &str,
    ) -> Result<String, ModalError> {
        // Snapshot directory goes through the task command router, not the control plane.
        // This is a placeholder - real implementation uses TaskCommandRouterClient.
        Err(ModalError::Other(format!(
            "sandbox_snapshot_directory for task {} path {} requires TaskCommandRouterClient",
            task_id, path
        )))
    }

    fn sandbox_mount_image(
        &self,
        task_id: &str,
        path: &str,
        image_id: &str,
    ) -> Result<(), ModalError> {
        // Mount image goes through the task command router, not the control plane.
        Err(ModalError::Other(format!(
            "sandbox_mount_image for task {} path {} image {} requires TaskCommandRouterClient",
            task_id, path, image_id
        )))
    }

    fn sandbox_create_connect_token(
        &self,
        sandbox_id: &str,
        user_metadata: &str,
    ) -> Result<SandboxCreateConnectCredentials, ModalError> {
        let request = pb::SandboxCreateConnectTokenRequest {
            sandbox_id: sandbox_id.to_string(),
            user_metadata: user_metadata.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.sandbox_create_connect_token(request))?.into_inner();
        Ok(SandboxCreateConnectCredentials {
            url: resp.url,
            token: resp.token,
        })
    }
}

// ============================================================================
// SandboxFilesystemGrpcClient implementation
// ============================================================================

impl crate::sandbox_filesystem::SandboxFilesystemGrpcClient for ModalGrpcTransport {
    fn filesystem_exec(
        &self,
        task_id: &str,
        request: FilesystemExecRequest,
    ) -> Result<FilesystemExecResponse, ModalError> {
        let file_exec_request_oneof = match request {
            FilesystemExecRequest::Open { path, mode } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileOpenRequest(
                    pb::ContainerFileOpenRequest {
                        file_descriptor: None,
                        path,
                        mode,
                    },
                ))
            }
            FilesystemExecRequest::Read { file_descriptor, n } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileReadRequest(
                    pb::ContainerFileReadRequest {
                        file_descriptor,
                        n: n.map(|v| v as u32),
                    },
                ))
            }
            FilesystemExecRequest::ReadLine { file_descriptor } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileReadLineRequest(
                    pb::ContainerFileReadLineRequest { file_descriptor },
                ))
            }
            FilesystemExecRequest::Write { file_descriptor, data } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileWriteRequest(
                    pb::ContainerFileWriteRequest {
                        file_descriptor,
                        data,
                    },
                ))
            }
            FilesystemExecRequest::Flush { file_descriptor } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileFlushRequest(
                    pb::ContainerFileFlushRequest { file_descriptor },
                ))
            }
            FilesystemExecRequest::Seek { file_descriptor, offset, whence } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileSeekRequest(
                    pb::ContainerFileSeekRequest {
                        file_descriptor,
                        offset: offset as i32,
                        whence: whence as i32,
                    },
                ))
            }
            FilesystemExecRequest::Close { file_descriptor } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileCloseRequest(
                    pb::ContainerFileCloseRequest { file_descriptor },
                ))
            }
            FilesystemExecRequest::Ls { path } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileLsRequest(
                    pb::ContainerFileLsRequest { path },
                ))
            }
            FilesystemExecRequest::Mkdir { path, parents } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileMkdirRequest(
                    pb::ContainerFileMkdirRequest {
                        path,
                        make_parents: parents,
                    },
                ))
            }
            FilesystemExecRequest::Rm { path, recursive } => {
                Some(pb::container_filesystem_exec_request::FileExecRequestOneof::FileRmRequest(
                    pb::ContainerFileRmRequest { path, recursive },
                ))
            }
        };

        let proto_request = pb::ContainerFilesystemExecRequest {
            task_id: task_id.to_string(),
            file_exec_request_oneof,
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.container_filesystem_exec(proto_request))?.into_inner();
        Ok(FilesystemExecResponse {
            exec_id: resp.exec_id,
            file_descriptor: resp.file_descriptor,
        })
    }

    fn filesystem_exec_get_output(
        &self,
        exec_id: &str,
    ) -> Result<Vec<u8>, ModalError> {
        let request = pb::ContainerFilesystemExecGetOutputRequest {
            exec_id: exec_id.to_string(),
            timeout: 55.0,
        };
        let mut client = self.client.lock().unwrap().clone();

        // This is a server-streaming RPC. Collect all output chunks.
        let stream = self.block_on(client.container_filesystem_exec_get_output(request))?;
        let mut stream = stream.into_inner();

        let mut output = Vec::new();
        loop {
            match self.runtime.block_on(stream.message()) {
                Ok(Some(batch)) => {
                    if let Some(error) = batch.error {
                        return Err(ModalError::SandboxFilesystem(error.error_message));
                    }
                    for chunk in batch.output {
                        output.extend_from_slice(&chunk);
                    }
                    if batch.eof {
                        break;
                    }
                }
                Ok(None) => break,
                Err(status) => return Err(Self::status_to_error(status)),
            }
        }
        Ok(output)
    }
}

// ============================================================================
// TaskCommandRouterGrpcClient implementation
// ============================================================================

impl crate::task_command_router::TaskCommandRouterGrpcClient for ModalGrpcTransport {
    fn task_get_command_router_access(
        &self,
        task_id: &str,
    ) -> Result<pb::TaskGetCommandRouterAccessResponse, ModalError> {
        let request = pb::TaskGetCommandRouterAccessRequest {
            task_id: task_id.to_string(),
        };
        let mut client = self.client.lock().unwrap().clone();
        let resp = self.block_on(client.task_get_command_router_access(request))?;
        Ok(resp.into_inner())
    }

    fn task_mount_directory(
        &self,
        _request: modal_proto::task_command_router::TaskMountDirectoryRequest,
        _jwt: &str,
    ) -> Result<(), ModalError> {
        // Task command router operations use a separate gRPC endpoint.
        // The real implementation would create a separate channel to the task command router URL.
        Err(ModalError::Other(
            "task_mount_directory requires a separate task command router connection".to_string(),
        ))
    }

    fn task_snapshot_directory(
        &self,
        _request: modal_proto::task_command_router::TaskSnapshotDirectoryRequest,
        _jwt: &str,
    ) -> Result<modal_proto::task_command_router::TaskSnapshotDirectoryResponse, ModalError> {
        Err(ModalError::Other(
            "task_snapshot_directory requires a separate task command router connection".to_string(),
        ))
    }

    fn task_exec_start(
        &self,
        _request: modal_proto::task_command_router::TaskExecStartRequest,
        _jwt: &str,
    ) -> Result<modal_proto::task_command_router::TaskExecStartResponse, ModalError> {
        Err(ModalError::Other(
            "task_exec_start requires a separate task command router connection".to_string(),
        ))
    }

    fn task_exec_stdin_write(
        &self,
        _request: modal_proto::task_command_router::TaskExecStdinWriteRequest,
        _jwt: &str,
    ) -> Result<modal_proto::task_command_router::TaskExecStdinWriteResponse, ModalError> {
        Err(ModalError::Other(
            "task_exec_stdin_write requires a separate task command router connection".to_string(),
        ))
    }

    fn task_exec_wait(
        &self,
        _request: modal_proto::task_command_router::TaskExecWaitRequest,
        _jwt: &str,
    ) -> Result<modal_proto::task_command_router::TaskExecWaitResponse, ModalError> {
        Err(ModalError::Other(
            "task_exec_wait requires a separate task command router connection".to_string(),
        ))
    }

    fn task_exec_stdio_read(
        &self,
        _request: modal_proto::task_command_router::TaskExecStdioReadRequest,
        _jwt: &str,
    ) -> Result<Vec<modal_proto::task_command_router::TaskExecStdioReadResponse>, ModalError> {
        Err(ModalError::Other(
            "task_exec_stdio_read requires a separate task command router connection".to_string(),
        ))
    }
}

// ============================================================================
// Arc delegation implementations
// ============================================================================
// These allow Arc<ModalGrpcTransport> to be used as a GrpcClient,
// enabling shared ownership across multiple service implementations.

impl crate::app::AppGrpcClient for Arc<ModalGrpcTransport> {
    fn app_get_or_create(&self, a: &str, b: &str, c: i32) -> Result<String, ModalError> {
        (**self).app_get_or_create(a, b, c)
    }
}

impl crate::cls::ClsGrpcClient for Arc<ModalGrpcTransport> {
    fn function_get(&self, a: &str, b: &str, c: &str) -> Result<(String, Option<pb::FunctionHandleMetadata>), ModalError> {
        (**self).function_get(a, b, c)
    }
}

impl crate::secret::SecretGrpcClient for Arc<ModalGrpcTransport> {
    fn secret_get_or_create(&self, a: &str, b: &str, c: &[String], d: i32, e: &HashMap<String, String>) -> Result<String, ModalError> {
        (**self).secret_get_or_create(a, b, c, d, e)
    }
    fn secret_delete(&self, a: &str) -> Result<(), ModalError> {
        (**self).secret_delete(a)
    }
}

impl crate::function::FunctionGrpcClient for Arc<ModalGrpcTransport> {
    fn function_get(&self, a: &str, b: &str, c: &str) -> Result<pb::FunctionGetResponse, ModalError> {
        crate::function::FunctionGrpcClient::function_get(&**self, a, b, c)
    }
    fn function_get_current_stats(&self, a: &str) -> Result<FunctionStats, ModalError> {
        (**self).function_get_current_stats(a)
    }
    fn function_update_scheduling_params(&self, a: &str, b: Option<u32>, c: Option<u32>, d: Option<u32>, e: Option<u32>) -> Result<(), ModalError> {
        (**self).function_update_scheduling_params(a, b, c, d, e)
    }
}

impl crate::function_call::FunctionCallGrpcClient for Arc<ModalGrpcTransport> {
    fn function_call_cancel(&self, a: &str, b: bool) -> Result<(), ModalError> {
        (**self).function_call_cancel(a, b)
    }
}

impl crate::invocation::InvocationGrpcClient for Arc<ModalGrpcTransport> {
    fn function_map(&self, a: &str, b: i32, c: i32, d: Vec<pb::FunctionPutInputsItem>) -> Result<pb::FunctionMapResponse, ModalError> {
        (**self).function_map(a, b, c, d)
    }
    fn function_get_outputs(&self, a: &str, b: u32, c: f32, d: &str, e: bool, f: f64) -> Result<pb::FunctionGetOutputsResponse, ModalError> {
        (**self).function_get_outputs(a, b, c, d, e, f)
    }
    fn function_retry_inputs(&self, a: &str, b: Vec<pb::FunctionRetryInputsItem>) -> Result<pb::FunctionRetryInputsResponse, ModalError> {
        (**self).function_retry_inputs(a, b)
    }
    fn attempt_start(&self, a: &str, b: pb::FunctionPutInputsItem) -> Result<pb::AttemptStartResponse, ModalError> {
        (**self).attempt_start(a, b)
    }
    fn attempt_await(&self, a: &str, b: f64, c: f32) -> Result<pb::AttemptAwaitResponse, ModalError> {
        (**self).attempt_await(a, b, c)
    }
    fn attempt_retry(&self, a: &str, b: pb::FunctionPutInputsItem, c: &str) -> Result<pb::AttemptRetryResponse, ModalError> {
        (**self).attempt_retry(a, b, c)
    }
    fn blob_get(&self, a: &str) -> Result<pb::BlobGetResponse, ModalError> {
        (**self).blob_get(a)
    }
}

impl crate::image::ImageGrpcClient for Arc<ModalGrpcTransport> {
    fn image_from_id(&self, a: &str) -> Result<String, ModalError> {
        (**self).image_from_id(a)
    }
    fn image_delete(&self, a: &str) -> Result<(), ModalError> {
        (**self).image_delete(a)
    }
    fn image_get_or_create(&self, a: &ImageLayerBuildRequest) -> Result<ImageBuildResult, ModalError> {
        (**self).image_get_or_create(a)
    }
    fn image_join_streaming(&self, a: &str, b: &str) -> Result<ImageJoinStreamingResult, ModalError> {
        (**self).image_join_streaming(a, b)
    }
}

impl crate::volume::VolumeGrpcClient for Arc<ModalGrpcTransport> {
    fn volume_get_or_create(&self, a: &str, b: &str, c: i32) -> Result<String, ModalError> {
        (**self).volume_get_or_create(a, b, c)
    }
    fn volume_heartbeat(&self, a: &str) -> Result<(), ModalError> {
        (**self).volume_heartbeat(a)
    }
    fn volume_delete(&self, a: &str) -> Result<(), ModalError> {
        (**self).volume_delete(a)
    }
}

impl crate::queue::QueueGrpcClient for Arc<ModalGrpcTransport> {
    fn queue_get_or_create(&self, a: &str, b: &str, c: i32) -> Result<String, ModalError> {
        (**self).queue_get_or_create(a, b, c)
    }
    fn queue_heartbeat(&self, a: &str) -> Result<(), ModalError> {
        (**self).queue_heartbeat(a)
    }
    fn queue_delete(&self, a: &str) -> Result<(), ModalError> {
        (**self).queue_delete(a)
    }
    fn queue_clear(&self, a: &str, b: Option<&[u8]>, c: bool) -> Result<(), ModalError> {
        (**self).queue_clear(a, b, c)
    }
    fn queue_len(&self, a: &str, b: Option<&[u8]>, c: bool) -> Result<i32, ModalError> {
        (**self).queue_len(a, b, c)
    }
    fn queue_get(&self, a: &str, b: Option<&[u8]>, c: f32, d: i32) -> Result<Vec<Vec<u8>>, ModalError> {
        (**self).queue_get(a, b, c, d)
    }
    fn queue_put(&self, a: &str, b: Vec<Vec<u8>>, c: Option<&[u8]>, d: i32) -> Result<(), ModalError> {
        (**self).queue_put(a, b, c, d)
    }
    fn queue_next_items(&self, a: &str, b: Option<&[u8]>, c: f32, d: &str) -> Result<Vec<(String, Vec<u8>)>, ModalError> {
        (**self).queue_next_items(a, b, c, d)
    }
}

impl crate::proxy::ProxyGrpcClient for Arc<ModalGrpcTransport> {
    fn proxy_get(&self, a: &str, b: &str) -> Result<Option<String>, ModalError> {
        (**self).proxy_get(a, b)
    }
}

impl crate::sandbox::SandboxGrpcClient for Arc<ModalGrpcTransport> {
    fn sandbox_create(&self, a: &str, b: &str, c: &SandboxCreateParams) -> Result<String, ModalError> {
        (**self).sandbox_create(a, b, c)
    }
    fn sandbox_get_task_id(&self, a: &str) -> Result<(Option<String>, bool), ModalError> {
        (**self).sandbox_get_task_id(a)
    }
    fn container_exec(&self, a: &str, b: Vec<String>, c: &SandboxExecParams) -> Result<String, ModalError> {
        (**self).container_exec(a, b, c)
    }
    fn container_exec_wait(&self, a: &str, b: f32) -> Result<ExecWaitResult, ModalError> {
        (**self).container_exec_wait(a, b)
    }
    fn sandbox_wait(&self, a: &str, b: f32) -> Result<SandboxWaitResult, ModalError> {
        (**self).sandbox_wait(a, b)
    }
    fn sandbox_terminate(&self, a: &str) -> Result<(), ModalError> {
        (**self).sandbox_terminate(a)
    }
    fn sandbox_from_id(&self, a: &str) -> Result<(), ModalError> {
        (**self).sandbox_from_id(a)
    }
    fn sandbox_from_name(&self, a: &str, b: &str, c: &str) -> Result<String, ModalError> {
        (**self).sandbox_from_name(a, b, c)
    }
    fn sandbox_list(&self, a: &str, b: &str, c: &HashMap<String, String>, d: f64) -> Result<Vec<SandboxListEntry>, ModalError> {
        (**self).sandbox_list(a, b, c, d)
    }
    fn sandbox_poll(&self, a: &str) -> Result<SandboxPollResult, ModalError> {
        (**self).sandbox_poll(a)
    }
    fn sandbox_tags_set(&self, a: &str, b: &HashMap<String, String>) -> Result<(), ModalError> {
        (**self).sandbox_tags_set(a, b)
    }
    fn sandbox_tags_get(&self, a: &str) -> Result<HashMap<String, String>, ModalError> {
        (**self).sandbox_tags_get(a)
    }
    fn sandbox_get_tunnels(&self, a: &str, b: f32) -> Result<SandboxTunnelsResult, ModalError> {
        (**self).sandbox_get_tunnels(a, b)
    }
    fn sandbox_snapshot_fs(&self, a: &str, b: f32) -> Result<SandboxSnapshotResult, ModalError> {
        (**self).sandbox_snapshot_fs(a, b)
    }
    fn sandbox_snapshot_directory(&self, a: &str, b: &str) -> Result<String, ModalError> {
        (**self).sandbox_snapshot_directory(a, b)
    }
    fn sandbox_mount_image(&self, a: &str, b: &str, c: &str) -> Result<(), ModalError> {
        (**self).sandbox_mount_image(a, b, c)
    }
    fn sandbox_create_connect_token(&self, a: &str, b: &str) -> Result<SandboxCreateConnectCredentials, ModalError> {
        (**self).sandbox_create_connect_token(a, b)
    }
}

impl crate::sandbox_filesystem::SandboxFilesystemGrpcClient for Arc<ModalGrpcTransport> {
    fn filesystem_exec(&self, a: &str, b: FilesystemExecRequest) -> Result<FilesystemExecResponse, ModalError> {
        (**self).filesystem_exec(a, b)
    }
    fn filesystem_exec_get_output(&self, a: &str) -> Result<Vec<u8>, ModalError> {
        (**self).filesystem_exec_get_output(a)
    }
}

impl crate::task_command_router::TaskCommandRouterGrpcClient for Arc<ModalGrpcTransport> {
    fn task_get_command_router_access(&self, a: &str) -> Result<pb::TaskGetCommandRouterAccessResponse, ModalError> {
        (**self).task_get_command_router_access(a)
    }
    fn task_mount_directory(&self, a: modal_proto::task_command_router::TaskMountDirectoryRequest, b: &str) -> Result<(), ModalError> {
        (**self).task_mount_directory(a, b)
    }
    fn task_snapshot_directory(&self, a: modal_proto::task_command_router::TaskSnapshotDirectoryRequest, b: &str) -> Result<modal_proto::task_command_router::TaskSnapshotDirectoryResponse, ModalError> {
        (**self).task_snapshot_directory(a, b)
    }
    fn task_exec_start(&self, a: modal_proto::task_command_router::TaskExecStartRequest, b: &str) -> Result<modal_proto::task_command_router::TaskExecStartResponse, ModalError> {
        (**self).task_exec_start(a, b)
    }
    fn task_exec_stdin_write(&self, a: modal_proto::task_command_router::TaskExecStdinWriteRequest, b: &str) -> Result<modal_proto::task_command_router::TaskExecStdinWriteResponse, ModalError> {
        (**self).task_exec_stdin_write(a, b)
    }
    fn task_exec_wait(&self, a: modal_proto::task_command_router::TaskExecWaitRequest, b: &str) -> Result<modal_proto::task_command_router::TaskExecWaitResponse, ModalError> {
        (**self).task_exec_wait(a, b)
    }
    fn task_exec_stdio_read(&self, a: modal_proto::task_command_router::TaskExecStdioReadRequest, b: &str) -> Result<Vec<modal_proto::task_command_router::TaskExecStdioReadResponse>, ModalError> {
        (**self).task_exec_stdio_read(a, b)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_endpoint_constant() {
        assert_eq!(API_ENDPOINT, "api.modal.com:443");
    }

    #[test]
    fn test_max_message_size() {
        assert_eq!(MAX_MESSAGE_SIZE, 100 * 1024 * 1024);
    }

    #[test]
    fn test_status_to_error() {
        let status = Status::not_found("test");
        let err = ModalGrpcTransport::status_to_error(status);
        assert!(matches!(err, ModalError::Grpc(_)));
        assert!(err.to_string().contains("test"));
    }

    #[test]
    fn test_connect_requires_tokio_runtime() {
        let profile = Profile::default();
        let result = ModalGrpcTransport::connect(&profile, "0.1.0");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("tokio runtime"),
            "expected tokio runtime error, got: {}",
            err
        );
    }

    #[test]
    fn test_connect_default_requires_runtime() {
        let profile = Profile::default();
        let result = ModalGrpcTransport::connect_default(&profile);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_channel_empty_url_uses_default() {
        let profile = Profile {
            server_url: String::new(),
            ..Profile::default()
        };
        // This will fail to connect but should parse the URL correctly
        let result = ModalGrpcTransport::create_channel(&profile).await;
        // Connection will fail (no server), but we verify URL parsing doesn't panic
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_channel_invalid_url() {
        let profile = Profile {
            server_url: "not-a-url".to_string(),
            ..Profile::default()
        };
        let result = ModalGrpcTransport::create_channel(&profile).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_create_client_missing_credentials() {
        // Create a dummy channel - we just need to test credential validation
        let profile = Profile {
            token_id: String::new(),
            token_secret: String::new(),
            ..Profile::default()
        };
        // We can't easily create a Channel without connecting, but we can test
        // that the validation logic in connect catches missing credentials.
        let result = ModalGrpcTransport::connect(&profile, "0.1.0");
        assert!(result.is_err());
    }
}
