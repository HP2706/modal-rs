use crate::app::AppService;
use crate::cloud_bucket_mount::{CloudBucketMountService, CloudBucketMountServiceImpl};
use crate::cls::ClsService;
use crate::config::Profile;
use crate::function::FunctionService;
use crate::function_call::FunctionCallService;
use crate::image::ImageService;
use crate::proxy::ProxyService;
use crate::queue::QueueService;
use crate::sandbox::SandboxService;
use crate::secret::SecretService;
use crate::volume::VolumeService;

/// Client exposes services for interacting with Modal resources.
/// Matches the Go SDK's Client struct with service accessors for all resource types.
pub struct Client {
    pub profile: Profile,
    pub sdk_version: String,
    pub apps: Box<dyn AppService>,
    pub cloud_bucket_mounts: Box<dyn CloudBucketMountService>,
    pub cls: Box<dyn ClsService>,
    pub functions: Box<dyn FunctionService>,
    pub function_calls: Box<dyn FunctionCallService>,
    pub images: Box<dyn ImageService>,
    pub proxies: Box<dyn ProxyService>,
    pub queues: Box<dyn QueueService>,
    pub sandboxes: Box<dyn SandboxService>,
    pub secrets: Box<dyn SecretService>,
    pub volumes: Box<dyn VolumeService>,
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("profile", &self.profile)
            .field("sdk_version", &self.sdk_version)
            .finish_non_exhaustive()
    }
}

/// ClientBuilder allows constructing a Client with custom service implementations.
/// This is primarily useful for testing with mock services.
pub struct ClientBuilder {
    profile: Profile,
    sdk_version: String,
    apps: Option<Box<dyn AppService>>,
    cloud_bucket_mounts: Option<Box<dyn CloudBucketMountService>>,
    cls: Option<Box<dyn ClsService>>,
    functions: Option<Box<dyn FunctionService>>,
    function_calls: Option<Box<dyn FunctionCallService>>,
    images: Option<Box<dyn ImageService>>,
    proxies: Option<Box<dyn ProxyService>>,
    queues: Option<Box<dyn QueueService>>,
    sandboxes: Option<Box<dyn SandboxService>>,
    secrets: Option<Box<dyn SecretService>>,
    volumes: Option<Box<dyn VolumeService>>,
}

impl ClientBuilder {
    pub fn new(profile: Profile) -> Self {
        Self {
            profile,
            sdk_version: "0.1.0".to_string(),
            apps: None,
            cloud_bucket_mounts: None,
            cls: None,
            functions: None,
            function_calls: None,
            images: None,
            proxies: None,
            queues: None,
            sandboxes: None,
            secrets: None,
            volumes: None,
        }
    }

    pub fn sdk_version(mut self, version: String) -> Self {
        self.sdk_version = version;
        self
    }

    pub fn apps(mut self, svc: Box<dyn AppService>) -> Self {
        self.apps = Some(svc);
        self
    }

    pub fn cloud_bucket_mounts(mut self, svc: Box<dyn CloudBucketMountService>) -> Self {
        self.cloud_bucket_mounts = Some(svc);
        self
    }

    pub fn cls(mut self, svc: Box<dyn ClsService>) -> Self {
        self.cls = Some(svc);
        self
    }

    pub fn functions(mut self, svc: Box<dyn FunctionService>) -> Self {
        self.functions = Some(svc);
        self
    }

    pub fn function_calls(mut self, svc: Box<dyn FunctionCallService>) -> Self {
        self.function_calls = Some(svc);
        self
    }

    pub fn images(mut self, svc: Box<dyn ImageService>) -> Self {
        self.images = Some(svc);
        self
    }

    pub fn proxies(mut self, svc: Box<dyn ProxyService>) -> Self {
        self.proxies = Some(svc);
        self
    }

    pub fn queues(mut self, svc: Box<dyn QueueService>) -> Self {
        self.queues = Some(svc);
        self
    }

    pub fn sandboxes(mut self, svc: Box<dyn SandboxService>) -> Self {
        self.sandboxes = Some(svc);
        self
    }

    pub fn secrets(mut self, svc: Box<dyn SecretService>) -> Self {
        self.secrets = Some(svc);
        self
    }

    pub fn volumes(mut self, svc: Box<dyn VolumeService>) -> Self {
        self.volumes = Some(svc);
        self
    }

    pub fn build(self) -> Result<Client, crate::error::ModalError> {
        Ok(Client {
            profile: self.profile,
            sdk_version: self.sdk_version,
            apps: self.apps.ok_or_else(|| {
                crate::error::ModalError::Other("apps service not configured".to_string())
            })?,
            cloud_bucket_mounts: self
                .cloud_bucket_mounts
                .unwrap_or_else(|| Box::new(CloudBucketMountServiceImpl)),
            cls: self.cls.ok_or_else(|| {
                crate::error::ModalError::Other("cls service not configured".to_string())
            })?,
            functions: self.functions.ok_or_else(|| {
                crate::error::ModalError::Other("functions service not configured".to_string())
            })?,
            function_calls: self.function_calls.ok_or_else(|| {
                crate::error::ModalError::Other(
                    "function_calls service not configured".to_string(),
                )
            })?,
            images: self.images.ok_or_else(|| {
                crate::error::ModalError::Other("images service not configured".to_string())
            })?,
            proxies: self.proxies.ok_or_else(|| {
                crate::error::ModalError::Other("proxies service not configured".to_string())
            })?,
            queues: self.queues.ok_or_else(|| {
                crate::error::ModalError::Other("queues service not configured".to_string())
            })?,
            sandboxes: self.sandboxes.ok_or_else(|| {
                crate::error::ModalError::Other("sandboxes service not configured".to_string())
            })?,
            secrets: self.secrets.ok_or_else(|| {
                crate::error::ModalError::Other("secrets service not configured".to_string())
            })?,
            volumes: self.volumes.ok_or_else(|| {
                crate::error::ModalError::Other("volumes service not configured".to_string())
            })?,
        })
    }
}

impl Client {
    /// Returns the SDK version.
    pub fn version(&self) -> &str {
        &self.sdk_version
    }
}

/// ClientParams defines credentials and options for initializing the Modal client.
#[derive(Debug, Clone, Default)]
pub struct ClientParams {
    pub token_id: String,
    pub token_secret: String,
    pub environment: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, AppFromNameParams};
    use crate::cls::{Cls, ClsFromNameParams};
    use crate::error::ModalError;
    use crate::function::{Function, FunctionFromNameParams};
    use crate::function_call::FunctionCall;
    use crate::image::{Image, ImageBuildParams, ImageDeleteParams, ImageFromRegistryParams};
    use crate::proxy::{Proxy, ProxyFromNameParams};
    use crate::queue::{Queue, QueueDeleteParams, QueueEphemeralParams, QueueFromNameParams};
    use crate::sandbox::{
        ExecWaitResult, Sandbox, SandboxCreateConnectCredentials,
        SandboxCreateConnectTokenParams, SandboxCreateParams, SandboxExecParams,
        SandboxFromNameParams, SandboxListParams, SandboxPollResult, SandboxWaitResult, Tunnel,
    };
    use std::collections::HashMap;
    use crate::secret::Secret;
    use crate::volume::{Volume, VolumeDeleteParams, VolumeEphemeralParams, VolumeFromNameParams};

    // Minimal mock implementations for each service trait

    struct MockAppService;
    impl AppService for MockAppService {
        fn from_name(&self, name: &str, _: Option<&AppFromNameParams>) -> Result<App, ModalError> {
            Ok(App {
                app_id: "ap-mock".to_string(),
                name: name.to_string(),
            })
        }
    }

    struct MockClsService;
    impl ClsService for MockClsService {
        fn from_name(
            &self,
            _: &str,
            _: &str,
            _: Option<&ClsFromNameParams>,
        ) -> Result<Cls, ModalError> {
            Ok(Cls {
                service_function_id: "fn-mock".to_string(),
                service_function_metadata: None,
                service_options: None,
            })
        }
    }

    struct MockFunctionService;
    impl FunctionService for MockFunctionService {
        fn from_name(
            &self,
            _: &str,
            _: &str,
            _: Option<&FunctionFromNameParams>,
        ) -> Result<Function, ModalError> {
            Ok(Function::new("fn-mock".to_string(), None))
        }
    }

    struct MockFunctionCallService;
    impl FunctionCallService for MockFunctionCallService {
        fn from_id(&self, id: &str) -> Result<FunctionCall, ModalError> {
            Ok(FunctionCall {
                function_call_id: id.to_string(),
            })
        }
    }

    struct MockImageService;
    impl ImageService for MockImageService {
        fn from_registry(&self, _: &str, _: Option<&ImageFromRegistryParams>) -> Image {
            Image::new("im-mock".to_string())
        }
        fn from_aws_ecr(&self, _: &str, _: &Secret) -> Image {
            Image::new("im-mock".to_string())
        }
        fn from_gcp_artifact_registry(&self, _: &str, _: &Secret) -> Image {
            Image::new("im-mock".to_string())
        }
        fn from_id(&self, _: &str) -> Result<Image, ModalError> {
            Ok(Image::new("im-mock".to_string()))
        }
        fn delete(&self, _: &str, _: Option<&ImageDeleteParams>) -> Result<(), ModalError> {
            Ok(())
        }
        fn build(&self, _: &Image, _: &ImageBuildParams) -> Result<Image, ModalError> {
            Ok(Image::new("im-built".to_string()))
        }
    }

    struct MockProxyService;
    impl ProxyService for MockProxyService {
        fn from_name(
            &self,
            _: &str,
            _: Option<&ProxyFromNameParams>,
        ) -> Result<Proxy, ModalError> {
            Ok(Proxy {
                proxy_id: "pr-mock".to_string(),
            })
        }
    }

    struct MockQueueService;
    impl QueueService for MockQueueService {
        fn from_name(
            &self,
            _: &str,
            _: Option<&QueueFromNameParams>,
        ) -> Result<Queue, ModalError> {
            Ok(Queue::new("qu-mock".to_string(), "test".to_string()))
        }
        fn ephemeral(&self, _: Option<&QueueEphemeralParams>) -> Result<Queue, ModalError> {
            Ok(Queue::new("qu-eph".to_string(), String::new()))
        }
        fn delete(&self, _: &str, _: Option<&QueueDeleteParams>) -> Result<(), ModalError> {
            Ok(())
        }
    }

    struct MockSandboxService;
    impl SandboxService for MockSandboxService {
        fn create(&self, _: &str, _: &str, _: SandboxCreateParams) -> Result<Sandbox, ModalError> {
            Ok(Sandbox::new("sb-mock".to_string()))
        }
        fn from_id(&self, _: &str) -> Result<Sandbox, ModalError> {
            Ok(Sandbox::new("sb-mock".to_string()))
        }
        fn from_name(&self, _: &str, _: &str, _: Option<&SandboxFromNameParams>) -> Result<Sandbox, ModalError> {
            Ok(Sandbox::new("sb-mock".to_string()))
        }
        fn list(&self, _: Option<&SandboxListParams>) -> Result<Vec<Sandbox>, ModalError> {
            Ok(vec![])
        }
        fn get_task_id(&self, _: &str) -> Result<String, ModalError> {
            Ok("ta-mock".to_string())
        }
        fn exec(&self, _: &Sandbox, _: Vec<String>, _: SandboxExecParams) -> Result<String, ModalError> {
            Ok("exec-mock".to_string())
        }
        fn exec_wait(&self, _: &str, _: f32) -> Result<ExecWaitResult, ModalError> {
            Ok(ExecWaitResult { exit_code: Some(0), completed: true })
        }
        fn wait(&self, _: &str, _: f32) -> Result<SandboxWaitResult, ModalError> {
            Ok(SandboxWaitResult { exit_code: 0, success: true, exception: None })
        }
        fn poll(&self, _: &str) -> Result<SandboxPollResult, ModalError> {
            Ok(SandboxPollResult { exit_code: None })
        }
        fn terminate(&self, _: &str) -> Result<(), ModalError> {
            Ok(())
        }
        fn set_tags(&self, _: &str, _: &HashMap<String, String>) -> Result<(), ModalError> {
            Ok(())
        }
        fn get_tags(&self, _: &str) -> Result<HashMap<String, String>, ModalError> {
            Ok(HashMap::new())
        }
        fn tunnels(&self, _: &str, _: f32) -> Result<HashMap<i32, Tunnel>, ModalError> {
            Ok(HashMap::new())
        }
        fn snapshot_filesystem(&self, _: &str, _: f32) -> Result<String, ModalError> {
            Ok("im-snap".to_string())
        }
        fn snapshot_directory(&self, _: &Sandbox, _: &str) -> Result<String, ModalError> {
            Ok("im-dir".to_string())
        }
        fn mount_image(&self, _: &Sandbox, _: &str, _: Option<&str>) -> Result<(), ModalError> {
            Ok(())
        }
        fn create_connect_token(&self, _: &str, _: Option<&SandboxCreateConnectTokenParams>) -> Result<SandboxCreateConnectCredentials, ModalError> {
            Ok(SandboxCreateConnectCredentials {
                token: "tok-mock".to_string(),
                url: "https://mock".to_string(),
            })
        }
    }

    struct MockSecretService;
    impl SecretService for MockSecretService {
        fn from_name(
            &self,
            name: &str,
            _: Option<&crate::secret::SecretFromNameParams>,
        ) -> Result<Secret, ModalError> {
            Ok(Secret {
                secret_id: "se-mock".to_string(),
                name: name.to_string(),
            })
        }
        fn from_map(
            &self,
            _: &std::collections::HashMap<String, String>,
            _: Option<&crate::secret::SecretFromMapParams>,
        ) -> Result<Secret, ModalError> {
            Ok(Secret {
                secret_id: "se-mock".to_string(),
                name: String::new(),
            })
        }
        fn delete(
            &self,
            _: &str,
            _: Option<&crate::secret::SecretDeleteParams>,
        ) -> Result<(), ModalError> {
            Ok(())
        }
    }

    struct MockVolumeService;
    impl VolumeService for MockVolumeService {
        fn from_name(
            &self,
            name: &str,
            _: Option<&VolumeFromNameParams>,
        ) -> Result<Volume, ModalError> {
            Ok(Volume::new("vo-mock".to_string(), name.to_string()))
        }
        fn ephemeral(&self, _: Option<&VolumeEphemeralParams>) -> Result<Volume, ModalError> {
            Ok(Volume::new("vo-eph".to_string(), String::new()))
        }
        fn delete(&self, _: &str, _: Option<&VolumeDeleteParams>) -> Result<(), ModalError> {
            Ok(())
        }
    }

    fn build_test_client() -> Client {
        ClientBuilder::new(Profile::default())
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

    #[test]
    fn test_client_builder_all_services() {
        let client = build_test_client();
        assert_eq!(client.version(), "0.1.0");
    }

    #[test]
    fn test_client_builder_missing_service() {
        let result = ClientBuilder::new(Profile::default()).build();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not configured"), "got: {}", err);
    }

    #[test]
    fn test_client_service_accessors() {
        let client = build_test_client();

        let app = client.apps.from_name("my-app", None).unwrap();
        assert_eq!(app.app_id, "ap-mock");

        let vol = client.volumes.from_name("my-vol", None).unwrap();
        assert_eq!(vol.volume_id, "vo-mock");

        let proxy = client.proxies.from_name("my-proxy", None).unwrap();
        assert_eq!(proxy.proxy_id, "pr-mock");

        let fc = client.function_calls.from_id("fc-123").unwrap();
        assert_eq!(fc.function_call_id, "fc-123");
    }

    #[test]
    fn test_client_debug() {
        let client = build_test_client();
        let debug_str = format!("{:?}", client);
        assert!(debug_str.contains("Client"), "got: {}", debug_str);
    }

    #[test]
    fn test_client_cloud_bucket_mounts_default() {
        let client = build_test_client();
        let mount = client
            .cloud_bucket_mounts
            .new_mount("my-bucket", None)
            .unwrap();
        assert_eq!(mount.bucket_name, "my-bucket");
    }

    #[test]
    fn test_client_builder_custom_version() {
        let client = ClientBuilder::new(Profile::default())
            .sdk_version("1.2.3".to_string())
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

        assert_eq!(client.version(), "1.2.3");
    }
}
