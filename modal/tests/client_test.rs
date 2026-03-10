#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Client.
/// Translated from libmodal/modal-go/client_test.go
///
/// Note: Go's client_test.go tests (TestClientWithLogger, TestClientWithCustomInterceptors)
/// require real gRPC connections. These tests verify equivalent behavior using mock-based
/// construction via ClientBuilder, which is the Rust-idiomatic approach.

use modal::client::{Client, ClientBuilder, ClientParams};
use modal::config::Profile;
use modal::error::ModalError;

use modal::app::{App, AppFromNameParams, AppService};
use modal::cls::{Cls, ClsFromNameParams, ClsService};
use modal::function::{Function, FunctionFromNameParams, FunctionService};
use modal::function_call::{FunctionCall, FunctionCallService};
use modal::image::{Image, ImageBuildParams, ImageDeleteParams, ImageFromRegistryParams, ImageService};
use modal::proxy::{Proxy, ProxyFromNameParams, ProxyService};
use modal::queue::{Queue, QueueDeleteParams, QueueEphemeralParams, QueueFromNameParams, QueueService};
use modal::sandbox::{
    ExecWaitResult, Sandbox, SandboxCreateConnectCredentials,
    SandboxCreateConnectTokenParams, SandboxCreateParams, SandboxExecParams,
    SandboxFromNameParams, SandboxListParams, SandboxPollResult, SandboxService,
    SandboxWaitResult, Tunnel,
};
use modal::secret::{Secret, SecretDeleteParams, SecretFromMapParams, SecretFromNameParams, SecretService};
use modal::volume::{Volume, VolumeDeleteParams, VolumeEphemeralParams, VolumeFromNameParams, VolumeService};
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Mock services (minimal implementations for testing Client construction)
// ---------------------------------------------------------------------------

struct MockAppService;
impl AppService for MockAppService {
    fn from_name(&self, name: &str, _: Option<&AppFromNameParams>) -> Result<App, ModalError> {
        Ok(App { app_id: "ap-test".to_string(), name: name.to_string() })
    }
}

struct MockClsService;
impl ClsService for MockClsService {
    fn from_name(&self, _: &str, _: &str, _: Option<&ClsFromNameParams>) -> Result<Cls, ModalError> {
        Ok(Cls { service_function_id: "fn-test".to_string(), service_function_metadata: None, service_options: None })
    }
}

struct MockFunctionService;
impl FunctionService for MockFunctionService {
    fn from_name(&self, _: &str, _: &str, _: Option<&FunctionFromNameParams>) -> Result<Function, ModalError> {
        Ok(Function::new("fn-test".to_string(), None))
    }
}

struct MockFunctionCallService;
impl FunctionCallService for MockFunctionCallService {
    fn from_id(&self, id: &str) -> Result<FunctionCall, ModalError> {
        Ok(FunctionCall { function_call_id: id.to_string() })
    }
}

struct MockImageService;
impl ImageService for MockImageService {
    fn from_registry(&self, _: &str, _: Option<&ImageFromRegistryParams>) -> Image { Image::new("im-test".to_string()) }
    fn from_aws_ecr(&self, _: &str, _: &Secret) -> Image { Image::new("im-test".to_string()) }
    fn from_gcp_artifact_registry(&self, _: &str, _: &Secret) -> Image { Image::new("im-test".to_string()) }
    fn from_id(&self, _: &str) -> Result<Image, ModalError> { Ok(Image::new("im-test".to_string())) }
    fn delete(&self, _: &str, _: Option<&ImageDeleteParams>) -> Result<(), ModalError> { Ok(()) }
    fn build(&self, _: &Image, _: &ImageBuildParams) -> Result<Image, ModalError> { Ok(Image::new("im-built".to_string())) }
}

struct MockProxyService;
impl ProxyService for MockProxyService {
    fn from_name(&self, _: &str, _: Option<&ProxyFromNameParams>) -> Result<Proxy, ModalError> {
        Ok(Proxy { proxy_id: "pr-test".to_string() })
    }
}

struct MockQueueService;
impl QueueService for MockQueueService {
    fn from_name(&self, _: &str, _: Option<&QueueFromNameParams>) -> Result<Queue, ModalError> {
        Ok(Queue::new("qu-test".to_string(), "test".to_string()))
    }
    fn ephemeral(&self, _: Option<&QueueEphemeralParams>) -> Result<Queue, ModalError> {
        Ok(Queue::new("qu-eph".to_string(), String::new()))
    }
    fn delete(&self, _: &str, _: Option<&QueueDeleteParams>) -> Result<(), ModalError> { Ok(()) }
}

struct MockSandboxService;
impl SandboxService for MockSandboxService {
    fn create(&self, _: &str, _: &str, _: SandboxCreateParams) -> Result<Sandbox, ModalError> { Ok(Sandbox::new("sb-test".to_string())) }
    fn from_id(&self, _: &str) -> Result<Sandbox, ModalError> { Ok(Sandbox::new("sb-test".to_string())) }
    fn from_name(&self, _: &str, _: &str, _: Option<&SandboxFromNameParams>) -> Result<Sandbox, ModalError> { Ok(Sandbox::new("sb-test".to_string())) }
    fn list(&self, _: Option<&SandboxListParams>) -> Result<Vec<Sandbox>, ModalError> { Ok(vec![]) }
    fn get_task_id(&self, _: &str) -> Result<String, ModalError> { Ok("ta-test".to_string()) }
    fn exec(&self, _: &Sandbox, _: Vec<String>, _: SandboxExecParams) -> Result<String, ModalError> { Ok("exec-test".to_string()) }
    fn exec_wait(&self, _: &str, _: f32) -> Result<ExecWaitResult, ModalError> { Ok(ExecWaitResult { exit_code: Some(0), completed: true }) }
    fn wait(&self, _: &str, _: f32) -> Result<SandboxWaitResult, ModalError> { Ok(SandboxWaitResult { exit_code: 0, success: true, exception: None }) }
    fn poll(&self, _: &str) -> Result<SandboxPollResult, ModalError> { Ok(SandboxPollResult { exit_code: None }) }
    fn terminate(&self, _: &str) -> Result<(), ModalError> { Ok(()) }
    fn set_tags(&self, _: &str, _: &HashMap<String, String>) -> Result<(), ModalError> { Ok(()) }
    fn get_tags(&self, _: &str) -> Result<HashMap<String, String>, ModalError> { Ok(HashMap::new()) }
    fn tunnels(&self, _: &str, _: f32) -> Result<HashMap<i32, Tunnel>, ModalError> { Ok(HashMap::new()) }
    fn snapshot_filesystem(&self, _: &str, _: f32) -> Result<String, ModalError> { Ok("im-snap".to_string()) }
    fn snapshot_directory(&self, _: &Sandbox, _: &str) -> Result<String, ModalError> { Ok("im-dir".to_string()) }
    fn mount_image(&self, _: &Sandbox, _: &str, _: Option<&str>) -> Result<(), ModalError> { Ok(()) }
    fn create_connect_token(&self, _: &str, _: Option<&SandboxCreateConnectTokenParams>) -> Result<SandboxCreateConnectCredentials, ModalError> {
        Ok(SandboxCreateConnectCredentials { token: "tok-test".to_string(), url: "https://test".to_string() })
    }
}

struct MockSecretService;
impl SecretService for MockSecretService {
    fn from_name(&self, name: &str, _: Option<&SecretFromNameParams>) -> Result<Secret, ModalError> {
        Ok(Secret { secret_id: "se-test".to_string(), name: name.to_string() })
    }
    fn from_map(&self, _: &HashMap<String, String>, _: Option<&SecretFromMapParams>) -> Result<Secret, ModalError> {
        Ok(Secret { secret_id: "se-test".to_string(), name: String::new() })
    }
    fn delete(&self, _: &str, _: Option<&SecretDeleteParams>) -> Result<(), ModalError> { Ok(()) }
}

struct MockVolumeService;
impl VolumeService for MockVolumeService {
    fn from_name(&self, name: &str, _: Option<&VolumeFromNameParams>) -> Result<Volume, ModalError> {
        Ok(Volume::new("vo-test".to_string(), name.to_string()))
    }
    fn ephemeral(&self, _: Option<&VolumeEphemeralParams>) -> Result<Volume, ModalError> {
        Ok(Volume::new("vo-eph".to_string(), String::new()))
    }
    fn delete(&self, _: &str, _: Option<&VolumeDeleteParams>) -> Result<(), ModalError> { Ok(()) }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

fn build_test_client() -> Client {
    build_test_client_with_profile(Profile::default())
}

fn build_test_client_with_profile(profile: Profile) -> Client {
    ClientBuilder::new(profile)
        .apps(Box::new(MockAppService))
        .cls(Box::new(MockClsService))
        .functions(Box::new(MockFunctionService))
        .function_calls(Box::new(MockFunctionCallService))
        .images(Box::new(MockImageService))
        .proxies(Box::new(MockProxyService))
        .queues(Box::new(MockQueueService))
        .sandboxes(Box::new(MockSandboxService))
        .secrets(Box::new(MockSecretService))
        .volumes(Box::new(MockVolumeService))
        .build()
        .unwrap()
}

// ---------------------------------------------------------------------------
// Tests — mirrors Go's client_test.go patterns
// ---------------------------------------------------------------------------

/// Equivalent to Go's TestClientWithLogger: verifies that a Client can be
/// initialized with a profile and the version is accessible.
/// (Logger integration in Rust uses the `log`/`tracing` ecosystem rather than
/// being injected per-client, so we test construction + version instead.)
#[test]
fn test_client_initialization() {
    let client = build_test_client();
    assert!(!client.version().is_empty());
    assert_eq!(client.version(), "0.1.0");
}

/// Verify that the profile is correctly stored and accessible on the client.
#[test]
fn test_client_stores_profile() {
    let profile = Profile {
        server_url: "https://custom.modal.com:443".to_string(),
        token_id: "tk-test123".to_string(),
        token_secret: "ts-secret456".to_string(),
        environment: "staging".to_string(),
        ..Default::default()
    };
    let client = build_test_client_with_profile(profile);
    assert_eq!(client.profile.server_url, "https://custom.modal.com:443");
    assert_eq!(client.profile.token_id, "tk-test123");
    assert_eq!(client.profile.environment, "staging");
}

/// Equivalent to Go's TestClientWithCustomInterceptors: verifies that all
/// 11 service accessors are functional and route requests to their service
/// implementations. (In Go this tests gRPC interceptors; in Rust the equivalent
/// extensibility point is the ClientBuilder with injectable services.)
#[test]
fn test_client_all_service_accessors() {
    let client = build_test_client();

    // Apps
    let app = client.apps.from_name("test-app", None).unwrap();
    assert_eq!(app.app_id, "ap-test");
    assert_eq!(app.name, "test-app");

    // Cls
    let cls = client.cls.from_name("test-app", "TestCls", None).unwrap();
    assert_eq!(cls.service_function_id, "fn-test");

    // Functions
    let func = client.functions.from_name("test-app", "test-fn", None).unwrap();
    assert_eq!(func.function_id, "fn-test");

    // FunctionCalls
    let fc = client.function_calls.from_id("fc-123").unwrap();
    assert_eq!(fc.function_call_id, "fc-123");

    // Images
    let img = client.images.from_registry("python:3.12", None);
    assert_eq!(img.image_id, "im-test");

    // Proxies
    let proxy = client.proxies.from_name("test-proxy", None).unwrap();
    assert_eq!(proxy.proxy_id, "pr-test");

    // Queues
    let queue = client.queues.from_name("test-queue", None).unwrap();
    assert_eq!(queue.queue_id, "qu-test");

    // Sandboxes
    let sb = client.sandboxes.from_id("sb-123").unwrap();
    assert_eq!(sb.sandbox_id, "sb-test");

    // Secrets
    let secret = client.secrets.from_name("test-secret", None).unwrap();
    assert_eq!(secret.secret_id, "se-test");

    // Volumes
    let vol = client.volumes.from_name("test-vol", None).unwrap();
    assert_eq!(vol.volume_id, "vo-test");

    // CloudBucketMounts (uses default implementation)
    let mount = client.cloud_bucket_mounts.new_mount("my-bucket", None).unwrap();
    assert_eq!(mount.bucket_name, "my-bucket");
}

/// Verify ClientBuilder errors when required services are not provided.
#[test]
fn test_client_builder_missing_required_services() {
    // No services set at all
    let result = ClientBuilder::new(Profile::default()).build();
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not configured"), "got: {}", err);

    // Only some services set
    let result = ClientBuilder::new(Profile::default())
        .apps(Box::new(MockAppService))
        .build();
    assert!(result.is_err());
}

/// Verify that custom SDK version is properly stored.
#[test]
fn test_client_custom_sdk_version() {
    let client = ClientBuilder::new(Profile::default())
        .sdk_version("2.0.0-beta".to_string())
        .apps(Box::new(MockAppService))
        .cls(Box::new(MockClsService))
        .functions(Box::new(MockFunctionService))
        .function_calls(Box::new(MockFunctionCallService))
        .images(Box::new(MockImageService))
        .proxies(Box::new(MockProxyService))
        .queues(Box::new(MockQueueService))
        .sandboxes(Box::new(MockSandboxService))
        .secrets(Box::new(MockSecretService))
        .volumes(Box::new(MockVolumeService))
        .build()
        .unwrap();

    assert_eq!(client.version(), "2.0.0-beta");
}

/// Verify that the client implements Debug for diagnostics.
#[test]
fn test_client_debug_representation() {
    let profile = Profile {
        server_url: "https://api.modal.com:443".to_string(),
        ..Default::default()
    };
    let client = build_test_client_with_profile(profile);
    let debug = format!("{:?}", client);
    assert!(debug.contains("Client"), "debug should contain 'Client', got: {}", debug);
    assert!(debug.contains("api.modal.com"), "debug should contain server URL, got: {}", debug);
}

/// Verify that two independent clients can coexist with different profiles.
#[test]
fn test_multiple_independent_clients() {
    let client1 = build_test_client_with_profile(Profile {
        environment: "production".to_string(),
        ..Default::default()
    });
    let client2 = build_test_client_with_profile(Profile {
        environment: "staging".to_string(),
        ..Default::default()
    });

    assert_eq!(client1.profile.environment, "production");
    assert_eq!(client2.profile.environment, "staging");

    // Both clients' services work independently
    let app1 = client1.apps.from_name("app1", None).unwrap();
    let app2 = client2.apps.from_name("app2", None).unwrap();
    assert_eq!(app1.name, "app1");
    assert_eq!(app2.name, "app2");
}

/// Test that ClientParams has sensible defaults.
#[test]
fn test_client_params_default() {
    let params = ClientParams::default();
    assert!(params.token_id.is_empty());
    assert!(params.token_secret.is_empty());
    assert!(params.environment.is_empty());
}
