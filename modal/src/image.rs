use crate::app::GpuConfig;
use crate::error::ModalError;
use crate::secret::Secret;

/// Registry authentication type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistryAuthType {
    StaticCreds,
    Aws,
    Gcp,
}

/// Image registry configuration for private registries.
#[derive(Debug, Clone)]
pub struct ImageRegistryConfig {
    pub registry_auth_type: RegistryAuthType,
    pub secret_id: String,
}

/// A single image layer with its build configuration.
#[derive(Debug, Clone, Default)]
pub struct Layer {
    pub commands: Vec<String>,
    pub env: std::collections::HashMap<String, String>,
    pub secrets: Vec<Secret>,
    pub gpu: String,
    pub force_build: bool,
}

/// Image represents a Modal Image, which can be used to create Sandboxes.
#[derive(Debug, Clone)]
pub struct Image {
    pub image_id: String,
    pub image_registry_config: Option<ImageRegistryConfig>,
    pub tag: String,
    pub layers: Vec<Layer>,
}

impl Image {
    /// Create a new Image with a given ID.
    pub fn new(image_id: String) -> Self {
        Self {
            image_id,
            image_registry_config: None,
            tag: String::new(),
            layers: vec![Layer::default()],
        }
    }

    /// DockerfileCommands extends an image with arbitrary Dockerfile-like commands.
    ///
    /// Each call creates a new Image layer that will be built sequentially.
    /// The provided options apply only to this layer.
    pub fn dockerfile_commands(
        &self,
        commands: &[String],
        params: Option<&ImageDockerfileCommandsParams>,
    ) -> Image {
        if commands.is_empty() {
            return self.clone();
        }

        let default_params = ImageDockerfileCommandsParams::default();
        let params = params.unwrap_or(&default_params);

        let new_layer = Layer {
            commands: commands.to_vec(),
            env: params.env.clone(),
            secrets: params.secrets.clone(),
            gpu: params.gpu.clone(),
            force_build: params.force_build,
        };

        let mut new_layers = self.layers.clone();
        new_layers.push(new_layer);

        Image {
            image_id: String::new(),
            tag: self.tag.clone(),
            image_registry_config: self.image_registry_config.clone(),
            layers: new_layers,
        }
    }
}

/// ImageFromRegistryParams are options for creating an Image from a registry.
#[derive(Debug, Clone, Default)]
pub struct ImageFromRegistryParams {
    /// Secret for private registry authentication.
    pub secret: Option<Secret>,
}

/// ImageDockerfileCommandsParams are options for Image.dockerfile_commands().
#[derive(Debug, Clone, Default)]
pub struct ImageDockerfileCommandsParams {
    /// Environment variables to set in the build environment.
    pub env: std::collections::HashMap<String, String>,
    /// Secrets available as environment variables to this layer's build environment.
    pub secrets: Vec<Secret>,
    /// GPU reservation for this layer's build environment (e.g. "A100", "T4:2").
    pub gpu: String,
    /// Ignore cached builds for this layer, similar to 'docker build --no-cache'.
    pub force_build: bool,
}

/// ImageDeleteParams are options for deleting an Image.
#[derive(Debug, Clone, Default)]
pub struct ImageDeleteParams;

/// Parameters for the ImageGetOrCreate RPC for a single layer.
#[derive(Debug, Clone)]
pub struct ImageLayerBuildRequest {
    pub app_id: String,
    pub dockerfile_commands: Vec<String>,
    pub image_registry_config: Option<ImageRegistryConfig>,
    pub secret_ids: Vec<String>,
    pub gpu_config: Option<GpuConfig>,
    pub base_images: Vec<BaseImage>,
    pub builder_version: String,
    pub force_build: bool,
}

/// A base image reference for multi-layer builds.
#[derive(Debug, Clone)]
pub struct BaseImage {
    pub docker_tag: String,
    pub image_id: String,
}

/// Parameters for building an image.
#[derive(Debug, Clone, Default)]
pub struct ImageBuildParams {
    pub app_id: String,
    pub builder_version: String,
}

/// Result status from an image build.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageBuildStatus {
    Success,
    Failure,
    Timeout,
    Terminated,
    Pending,
}

/// Result of an image build operation.
#[derive(Debug, Clone)]
pub struct ImageBuildResult {
    pub image_id: String,
    pub status: ImageBuildStatus,
    pub exception: Option<String>,
}

impl ImageBuildResult {
    /// Convert the build result into a Result, returning an error if the build failed.
    pub fn into_result(self) -> Result<String, ModalError> {
        match self.status {
            ImageBuildStatus::Success => Ok(self.image_id),
            ImageBuildStatus::Failure => Err(ModalError::Remote(format!(
                "Image build for {} failed with the exception:\n{}",
                self.image_id,
                self.exception.unwrap_or_else(|| "unknown error".to_string())
            ))),
            ImageBuildStatus::Terminated => Err(ModalError::Remote(format!(
                "Image build for {} terminated due to external shut-down. Please try again.",
                self.image_id
            ))),
            ImageBuildStatus::Timeout => Err(ModalError::Remote(format!(
                "Image build for {} timed out. Please try again with a larger timeout parameter.",
                self.image_id
            ))),
            ImageBuildStatus::Pending => Err(ModalError::Other(
                "image build still pending".to_string(),
            )),
        }
    }
}

/// ImageService provides Image related operations.
pub trait ImageService: Send + Sync {
    fn from_registry(&self, tag: &str, params: Option<&ImageFromRegistryParams>) -> Image;
    fn from_aws_ecr(&self, tag: &str, secret: &Secret) -> Image;
    fn from_gcp_artifact_registry(&self, tag: &str, secret: &Secret) -> Image;
    fn from_id(&self, image_id: &str) -> Result<Image, ModalError>;
    fn delete(&self, image_id: &str, params: Option<&ImageDeleteParams>) -> Result<(), ModalError>;

    /// Build an image layer-by-layer. Returns the built Image with image_id set.
    fn build(&self, image: &Image, params: &ImageBuildParams) -> Result<Image, ModalError>;
}

/// Trait abstracting the gRPC calls needed for Image operations.
pub trait ImageGrpcClient: Send + Sync {
    fn image_from_id(&self, image_id: &str) -> Result<String, ModalError>;
    fn image_delete(&self, image_id: &str) -> Result<(), ModalError>;

    /// Create or find an existing image layer (ImageGetOrCreate RPC).
    /// Returns (image_id, optional build result).
    fn image_get_or_create(
        &self,
        request: &ImageLayerBuildRequest,
    ) -> Result<ImageBuildResult, ModalError>;

    /// Poll for image build completion (ImageJoinStreaming RPC).
    /// Polls until a terminal result is received.
    /// The last_entry_id is used for resumable polling.
    fn image_join_streaming(
        &self,
        image_id: &str,
        last_entry_id: &str,
    ) -> Result<ImageJoinStreamingResult, ModalError>;
}

/// Result from a single image_join_streaming poll iteration.
#[derive(Debug, Clone)]
pub struct ImageJoinStreamingResult {
    /// The build result, if the build has completed. None if the stream ended without a result.
    pub result: Option<ImageBuildResult>,
    /// The last entry ID for resumable polling.
    pub last_entry_id: String,
}

/// Implementation of ImageService backed by a gRPC client.
pub struct ImageServiceImpl<C: ImageGrpcClient> {
    pub client: C,
}

impl<C: ImageGrpcClient> ImageService for ImageServiceImpl<C> {
    fn from_registry(&self, tag: &str, params: Option<&ImageFromRegistryParams>) -> Image {
        let default_params = ImageFromRegistryParams::default();
        let params = params.unwrap_or(&default_params);

        let image_registry_config = params.secret.as_ref().map(|s| ImageRegistryConfig {
            registry_auth_type: RegistryAuthType::StaticCreds,
            secret_id: s.secret_id.clone(),
        });

        Image {
            image_id: String::new(),
            image_registry_config,
            tag: tag.to_string(),
            layers: vec![Layer::default()],
        }
    }

    fn from_aws_ecr(&self, tag: &str, secret: &Secret) -> Image {
        Image {
            image_id: String::new(),
            image_registry_config: Some(ImageRegistryConfig {
                registry_auth_type: RegistryAuthType::Aws,
                secret_id: secret.secret_id.clone(),
            }),
            tag: tag.to_string(),
            layers: vec![Layer::default()],
        }
    }

    fn from_gcp_artifact_registry(&self, tag: &str, secret: &Secret) -> Image {
        Image {
            image_id: String::new(),
            image_registry_config: Some(ImageRegistryConfig {
                registry_auth_type: RegistryAuthType::Gcp,
                secret_id: secret.secret_id.clone(),
            }),
            tag: tag.to_string(),
            layers: vec![Layer::default()],
        }
    }

    fn from_id(&self, image_id: &str) -> Result<Image, ModalError> {
        let resolved_id = self.client.image_from_id(image_id).map_err(|e| {
            if matches!(&e, ModalError::Grpc(s) if s.code() == tonic::Code::NotFound) {
                ModalError::NotFound(format!("Image '{}' not found", image_id))
            } else {
                e
            }
        })?;

        Ok(Image {
            image_id: resolved_id,
            image_registry_config: None,
            tag: String::new(),
            layers: vec![Layer::default()],
        })
    }

    fn delete(&self, image_id: &str, _params: Option<&ImageDeleteParams>) -> Result<(), ModalError> {
        self.client.image_delete(image_id).map_err(|e| {
            if matches!(&e, ModalError::Grpc(s) if s.code() == tonic::Code::NotFound) {
                ModalError::NotFound(format!("Image '{}' not found", image_id))
            } else {
                e
            }
        })
    }

    fn build(&self, image: &Image, params: &ImageBuildParams) -> Result<Image, ModalError> {
        // Image is already hydrated
        if !image.image_id.is_empty() {
            return Ok(image.clone());
        }

        // Validate all layers before building
        for layer in &image.layers {
            validate_dockerfile_commands(&layer.commands)?;
        }

        let builder_version = if params.builder_version.is_empty() {
            "2024.10".to_string()
        } else {
            params.builder_version.clone()
        };

        let mut current_image_id = String::new();

        for (i, current_layer) in image.layers.iter().enumerate() {
            // Collect secret IDs from layer secrets
            let secret_ids: Vec<String> = current_layer
                .secrets
                .iter()
                .map(|s| s.secret_id.clone())
                .collect();

            // Parse GPU config if specified
            let gpu_config = if current_layer.gpu.is_empty() {
                None
            } else {
                let config = crate::app::parse_gpu_config(&current_layer.gpu)?;
                Some(config)
            };

            // Build dockerfile commands and base images based on layer position
            let (dockerfile_commands, base_images) = if i == 0 {
                // First layer: FROM <tag>
                let mut cmds = vec![format!("FROM {}", image.tag)];
                cmds.extend(current_layer.commands.iter().cloned());
                (cmds, vec![])
            } else {
                // Subsequent layers: FROM base, with base image linking
                let mut cmds = vec!["FROM base".to_string()];
                cmds.extend(current_layer.commands.iter().cloned());
                let base = BaseImage {
                    docker_tag: "base".to_string(),
                    image_id: current_image_id.clone(),
                };
                (cmds, vec![base])
            };

            // Determine force_build: inherit from previous layers if any layer requested it
            let force_build = current_layer.force_build
                || (i > 0 && image.layers[..i].iter().any(|l| l.force_build));

            let request = ImageLayerBuildRequest {
                app_id: params.app_id.clone(),
                dockerfile_commands,
                image_registry_config: if i == 0 {
                    image.image_registry_config.clone()
                } else {
                    None
                },
                secret_ids,
                gpu_config,
                base_images,
                builder_version: builder_version.clone(),
                force_build,
            };

            let build_result = self.client.image_get_or_create(&request)?;

            let final_result = if build_result.status == ImageBuildStatus::Pending {
                // Build is in progress — poll until complete
                let mut last_entry_id = String::new();
                loop {
                    let join_result = self.client.image_join_streaming(
                        &build_result.image_id,
                        &last_entry_id,
                    )?;
                    last_entry_id = join_result.last_entry_id;
                    if let Some(result) = join_result.result {
                        break result;
                    }
                    // Stream ended without result, retry
                }
            } else {
                build_result
            };

            // Check the result status
            current_image_id = final_result.into_result()?;
        }

        Ok(Image {
            image_id: current_image_id,
            image_registry_config: image.image_registry_config.clone(),
            tag: image.tag.clone(),
            layers: image.layers.clone(),
        })
    }
}

/// Validate that Dockerfile commands don't contain unsupported COPY operations.
pub fn validate_dockerfile_commands(commands: &[String]) -> Result<(), ModalError> {
    for command in commands {
        let trimmed = command.trim().to_uppercase();
        if trimmed.starts_with("COPY ") && !trimmed.starts_with("COPY --FROM=") {
            return Err(ModalError::Invalid(
                "COPY commands that copy from local context are not yet supported.".to_string(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Mutex;

    /// Recorded request from image_get_or_create calls for test assertions.
    #[derive(Debug, Clone)]
    struct RecordedLayerRequest {
        dockerfile_commands: Vec<String>,
        secret_ids: Vec<String>,
        gpu_config: Option<GpuConfig>,
        base_images: Vec<BaseImage>,
        force_build: bool,
    }

    struct MockImageGrpcClient {
        from_id_result: Result<String, ModalError>,
        delete_result: Result<(), ModalError>,
        build_responses: Mutex<Vec<MockBuildResponse>>,
        recorded_requests: Mutex<Vec<RecordedLayerRequest>>,
    }

    enum MockBuildResponse {
        GetOrCreate(Result<ImageBuildResult, ModalError>),
        JoinStreaming(Result<ImageJoinStreamingResult, ModalError>),
    }

    impl ImageGrpcClient for MockImageGrpcClient {
        fn image_from_id(&self, _image_id: &str) -> Result<String, ModalError> {
            match &self.from_id_result {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(ModalError::Other(e.to_string())),
            }
        }

        fn image_delete(&self, _image_id: &str) -> Result<(), ModalError> {
            match &self.delete_result {
                Ok(()) => Ok(()),
                Err(e) => Err(ModalError::Other(e.to_string())),
            }
        }

        fn image_get_or_create(
            &self,
            request: &ImageLayerBuildRequest,
        ) -> Result<ImageBuildResult, ModalError> {
            // Record the request for assertions
            self.recorded_requests.lock().unwrap().push(RecordedLayerRequest {
                dockerfile_commands: request.dockerfile_commands.clone(),
                secret_ids: request.secret_ids.clone(),
                gpu_config: request.gpu_config.clone(),
                base_images: request.base_images.clone(),
                force_build: request.force_build,
            });

            let mut responses = self.build_responses.lock().unwrap();
            match responses.remove(0) {
                MockBuildResponse::GetOrCreate(r) => r,
                _ => panic!("expected GetOrCreate mock response"),
            }
        }

        fn image_join_streaming(
            &self,
            _image_id: &str,
            _last_entry_id: &str,
        ) -> Result<ImageJoinStreamingResult, ModalError> {
            let mut responses = self.build_responses.lock().unwrap();
            match responses.remove(0) {
                MockBuildResponse::JoinStreaming(r) => r,
                _ => panic!("expected JoinStreaming mock response"),
            }
        }
    }

    fn make_service(
        from_id_result: Result<String, ModalError>,
        delete_result: Result<(), ModalError>,
    ) -> ImageServiceImpl<MockImageGrpcClient> {
        ImageServiceImpl {
            client: MockImageGrpcClient {
                from_id_result,
                delete_result,
                build_responses: Mutex::new(Vec::new()),
                recorded_requests: Mutex::new(Vec::new()),
            },
        }
    }

    fn make_build_service(
        responses: Vec<MockBuildResponse>,
    ) -> ImageServiceImpl<MockImageGrpcClient> {
        ImageServiceImpl {
            client: MockImageGrpcClient {
                from_id_result: Ok(String::new()),
                delete_result: Ok(()),
                build_responses: Mutex::new(responses),
                recorded_requests: Mutex::new(Vec::new()),
            },
        }
    }

    #[test]
    fn test_from_registry_public() {
        let svc = make_service(Ok(String::new()), Ok(()));
        let image = svc.from_registry("python:3.12-slim", None);

        assert_eq!(image.tag, "python:3.12-slim");
        assert_eq!(image.image_id, "");
        assert!(image.image_registry_config.is_none());
        assert_eq!(image.layers.len(), 1);
    }

    #[test]
    fn test_from_registry_private() {
        let svc = make_service(Ok(String::new()), Ok(()));
        let secret = Secret {
            secret_id: "st-secret-123".to_string(),
            name: "my-secret".to_string(),
        };
        let image = svc.from_registry(
            "private.registry.io/my-image:latest",
            Some(&ImageFromRegistryParams {
                secret: Some(secret),
            }),
        );

        assert_eq!(image.tag, "private.registry.io/my-image:latest");
        let config = image.image_registry_config.unwrap();
        assert_eq!(config.registry_auth_type, RegistryAuthType::StaticCreds);
        assert_eq!(config.secret_id, "st-secret-123");
    }

    #[test]
    fn test_from_aws_ecr() {
        let svc = make_service(Ok(String::new()), Ok(()));
        let secret = Secret {
            secret_id: "st-aws-123".to_string(),
            name: "aws-creds".to_string(),
        };
        let image = svc.from_aws_ecr("123456789.dkr.ecr.us-east-1.amazonaws.com/my-image", &secret);

        let config = image.image_registry_config.unwrap();
        assert_eq!(config.registry_auth_type, RegistryAuthType::Aws);
        assert_eq!(config.secret_id, "st-aws-123");
    }

    #[test]
    fn test_from_gcp_artifact_registry() {
        let svc = make_service(Ok(String::new()), Ok(()));
        let secret = Secret {
            secret_id: "st-gcp-123".to_string(),
            name: "gcp-creds".to_string(),
        };
        let image = svc.from_gcp_artifact_registry("us-docker.pkg.dev/project/repo/image", &secret);

        let config = image.image_registry_config.unwrap();
        assert_eq!(config.registry_auth_type, RegistryAuthType::Gcp);
        assert_eq!(config.secret_id, "st-gcp-123");
    }

    #[test]
    fn test_from_id() {
        let svc = make_service(Ok("im-resolved-456".to_string()), Ok(()));
        let image = svc.from_id("im-test-123").unwrap();
        assert_eq!(image.image_id, "im-resolved-456");
    }

    #[test]
    fn test_delete() {
        let svc = make_service(Ok(String::new()), Ok(()));
        svc.delete("im-test-123", None).unwrap();
    }

    #[test]
    fn test_dockerfile_commands() {
        let base = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "python:3.12".to_string(),
            layers: vec![Layer::default()],
        };

        let extended = base.dockerfile_commands(
            &[
                "RUN pip install numpy".to_string(),
                "RUN pip install pandas".to_string(),
            ],
            None,
        );

        assert_eq!(extended.layers.len(), 2);
        assert_eq!(extended.layers[1].commands.len(), 2);
        assert_eq!(extended.layers[1].commands[0], "RUN pip install numpy");
        assert_eq!(extended.tag, "python:3.12");
        assert_eq!(extended.image_id, "");
    }

    #[test]
    fn test_dockerfile_commands_empty_returns_same() {
        let base = Image::new("im-test".to_string());
        let result = base.dockerfile_commands(&[], None);
        assert_eq!(result.image_id, "im-test");
        assert_eq!(result.layers.len(), 1);
    }

    #[test]
    fn test_dockerfile_commands_with_params() {
        let base = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "python:3.12".to_string(),
            layers: vec![Layer::default()],
        };

        let mut env = std::collections::HashMap::new();
        env.insert("MY_VAR".to_string(), "value".to_string());

        let extended = base.dockerfile_commands(
            &["RUN echo $MY_VAR".to_string()],
            Some(&ImageDockerfileCommandsParams {
                env,
                gpu: "A100".to_string(),
                force_build: true,
                ..Default::default()
            }),
        );

        assert_eq!(extended.layers.len(), 2);
        assert_eq!(extended.layers[1].gpu, "A100");
        assert!(extended.layers[1].force_build);
        assert_eq!(extended.layers[1].env.get("MY_VAR").unwrap(), "value");
    }

    #[test]
    fn test_dockerfile_commands_chaining() {
        let base = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "python:3.12".to_string(),
            layers: vec![Layer::default()],
        };

        let step1 = base.dockerfile_commands(&["RUN pip install a".to_string()], None);
        let step2 = step1.dockerfile_commands(&["RUN pip install b".to_string()], None);

        assert_eq!(step2.layers.len(), 3);
    }

    #[test]
    fn test_validate_dockerfile_commands_valid() {
        validate_dockerfile_commands(&[
            "RUN pip install numpy".to_string(),
            "ENV MY_VAR=value".to_string(),
            "COPY --from=builder /app /app".to_string(),
        ])
        .unwrap();
    }

    #[test]
    fn test_validate_dockerfile_commands_invalid_copy() {
        let err = validate_dockerfile_commands(&["COPY . /app".to_string()]).unwrap_err();
        assert!(
            err.to_string()
                .contains("COPY commands that copy from local context are not yet supported"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_validate_dockerfile_commands_copy_from_ok() {
        validate_dockerfile_commands(&["COPY --from=builder /app /app".to_string()]).unwrap();
    }

    // --- ImageBuildResult tests ---

    #[test]
    fn test_build_result_success() {
        let result = ImageBuildResult {
            image_id: "im-built-123".to_string(),
            status: ImageBuildStatus::Success,
            exception: None,
        };
        assert_eq!(result.into_result().unwrap(), "im-built-123");
    }

    #[test]
    fn test_build_result_failure() {
        let result = ImageBuildResult {
            image_id: "im-failed".to_string(),
            status: ImageBuildStatus::Failure,
            exception: Some("build step failed".to_string()),
        };
        let err = result.into_result().unwrap_err();
        assert!(err.to_string().contains("build step failed"));
    }

    #[test]
    fn test_build_result_timeout() {
        let result = ImageBuildResult {
            image_id: "im-timeout".to_string(),
            status: ImageBuildStatus::Timeout,
            exception: None,
        };
        let err = result.into_result().unwrap_err();
        assert!(err.to_string().contains("timed out"));
        assert!(err.to_string().contains("im-timeout"));
    }

    #[test]
    fn test_build_result_terminated() {
        let result = ImageBuildResult {
            image_id: "im-terminated".to_string(),
            status: ImageBuildStatus::Terminated,
            exception: None,
        };
        let err = result.into_result().unwrap_err();
        assert!(err.to_string().contains("terminated"));
        assert!(err.to_string().contains("im-terminated"));
    }

    #[test]
    fn test_build_result_pending() {
        let result = ImageBuildResult {
            image_id: "im-pending".to_string(),
            status: ImageBuildStatus::Pending,
            exception: None,
        };
        let err = result.into_result().unwrap_err();
        assert!(err.to_string().contains("pending"));
    }

    // --- Image build: already hydrated ---

    #[test]
    fn test_build_already_hydrated() {
        let svc = make_build_service(vec![]);
        let image = Image::new("im-already-built".to_string());
        let built = svc.build(&image, &ImageBuildParams::default()).unwrap();
        assert_eq!(built.image_id, "im-already-built");
    }

    // --- Image build: single layer ---

    #[test]
    fn test_build_single_layer_cached() {
        let svc = make_build_service(vec![MockBuildResponse::GetOrCreate(Ok(
            ImageBuildResult {
                image_id: "im-cached-1".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            },
        ))]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let built = svc.build(&image, &ImageBuildParams::default()).unwrap();
        assert_eq!(built.image_id, "im-cached-1");

        // Verify the request
        let requests = svc.client.recorded_requests.lock().unwrap();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].dockerfile_commands, vec!["FROM alpine:3.21"]);
        assert!(requests[0].base_images.is_empty());
        assert!(requests[0].secret_ids.is_empty());
        assert!(requests[0].gpu_config.is_none());
        assert!(!requests[0].force_build);
    }

    #[test]
    fn test_build_single_layer_pending_then_success() {
        let svc = make_build_service(vec![
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-building-1".to_string(),
                status: ImageBuildStatus::Pending,
                exception: None,
            })),
            MockBuildResponse::JoinStreaming(Ok(ImageJoinStreamingResult {
                result: Some(ImageBuildResult {
                    image_id: "im-building-1".to_string(),
                    status: ImageBuildStatus::Success,
                    exception: None,
                }),
                last_entry_id: "entry-1".to_string(),
            })),
        ]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let built = svc.build(&image, &ImageBuildParams::default()).unwrap();
        assert_eq!(built.image_id, "im-building-1");
    }

    #[test]
    fn test_build_pending_retry_then_success() {
        // Stream ends without result first time, then succeeds
        let svc = make_build_service(vec![
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-retry-1".to_string(),
                status: ImageBuildStatus::Pending,
                exception: None,
            })),
            MockBuildResponse::JoinStreaming(Ok(ImageJoinStreamingResult {
                result: None,
                last_entry_id: "entry-1".to_string(),
            })),
            MockBuildResponse::JoinStreaming(Ok(ImageJoinStreamingResult {
                result: Some(ImageBuildResult {
                    image_id: "im-retry-1".to_string(),
                    status: ImageBuildStatus::Success,
                    exception: None,
                }),
                last_entry_id: "entry-2".to_string(),
            })),
        ]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let built = svc.build(&image, &ImageBuildParams::default()).unwrap();
        assert_eq!(built.image_id, "im-retry-1");
    }

    #[test]
    fn test_build_pending_then_failure() {
        let svc = make_build_service(vec![
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-building-2".to_string(),
                status: ImageBuildStatus::Pending,
                exception: None,
            })),
            MockBuildResponse::JoinStreaming(Ok(ImageJoinStreamingResult {
                result: Some(ImageBuildResult {
                    image_id: "im-building-2".to_string(),
                    status: ImageBuildStatus::Failure,
                    exception: Some("pip install failed".to_string()),
                }),
                last_entry_id: String::new(),
            })),
        ]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("pip install failed"));
    }

    #[test]
    fn test_build_immediate_failure() {
        let svc = make_build_service(vec![MockBuildResponse::GetOrCreate(Ok(
            ImageBuildResult {
                image_id: "im-fail".to_string(),
                status: ImageBuildStatus::Failure,
                exception: Some("invalid dockerfile".to_string()),
            },
        ))]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("invalid dockerfile"));
    }

    #[test]
    fn test_build_grpc_error() {
        let svc = make_build_service(vec![MockBuildResponse::GetOrCreate(Err(
            ModalError::Grpc(tonic::Status::unavailable("server down")),
        ))]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(matches!(err, ModalError::Grpc(_)));
    }

    // --- Multi-layer build tests ---

    #[test]
    fn test_build_multi_layer_all_cached() {
        // Matches Go's TestDockerfileCommandsWithOptions test structure
        let svc = make_build_service(vec![
            // Layer 0: FROM alpine:3.21
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-base".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
            // Layer 1: FROM base + RUN echo layer1
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-layer1".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
            // Layer 2: FROM base + RUN echo layer2 (with secrets, GPU, force_build)
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-layer2".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
            // Layer 3: FROM base + RUN echo layer3 (inherits force_build)
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-layer3".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
        ]);

        let secret = Secret {
            secret_id: "sc-test".to_string(),
            name: "test".to_string(),
        };

        let image = svc
            .from_registry("alpine:3.21", None)
            .dockerfile_commands(&["RUN echo layer1".to_string()], None)
            .dockerfile_commands(
                &["RUN echo layer2".to_string()],
                Some(&ImageDockerfileCommandsParams {
                    secrets: vec![secret],
                    gpu: "A100".to_string(),
                    force_build: true,
                    ..Default::default()
                }),
            )
            .dockerfile_commands(
                &["RUN echo layer3".to_string()],
                Some(&ImageDockerfileCommandsParams {
                    force_build: true,
                    ..Default::default()
                }),
            );

        let built = svc
            .build(
                &image,
                &ImageBuildParams {
                    app_id: "ap-test".to_string(),
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(built.image_id, "im-layer3");

        // Verify all recorded requests
        let requests = svc.client.recorded_requests.lock().unwrap();
        assert_eq!(requests.len(), 4);

        // Layer 0: FROM alpine:3.21, no base images, no secrets, no GPU
        assert_eq!(requests[0].dockerfile_commands, vec!["FROM alpine:3.21"]);
        assert!(requests[0].base_images.is_empty());
        assert!(requests[0].secret_ids.is_empty());
        assert!(requests[0].gpu_config.is_none());
        assert!(!requests[0].force_build);

        // Layer 1: FROM base + commands, base image points to im-base
        assert_eq!(
            requests[1].dockerfile_commands,
            vec!["FROM base", "RUN echo layer1"]
        );
        assert_eq!(requests[1].base_images.len(), 1);
        assert_eq!(requests[1].base_images[0].docker_tag, "base");
        assert_eq!(requests[1].base_images[0].image_id, "im-base");
        assert!(requests[1].secret_ids.is_empty());
        assert!(requests[1].gpu_config.is_none());
        assert!(!requests[1].force_build);

        // Layer 2: FROM base + commands, with secrets, GPU, force_build
        assert_eq!(
            requests[2].dockerfile_commands,
            vec!["FROM base", "RUN echo layer2"]
        );
        assert_eq!(requests[2].base_images.len(), 1);
        assert_eq!(requests[2].base_images[0].image_id, "im-layer1");
        assert_eq!(requests[2].secret_ids, vec!["sc-test"]);
        assert!(requests[2].gpu_config.is_some());
        let gpu = requests[2].gpu_config.as_ref().unwrap();
        assert_eq!(gpu.gpu_type, "A100");
        assert_eq!(gpu.count, 1);
        assert!(requests[2].force_build);

        // Layer 3: FROM base + commands, inherits force_build
        assert_eq!(
            requests[3].dockerfile_commands,
            vec!["FROM base", "RUN echo layer3"]
        );
        assert_eq!(requests[3].base_images.len(), 1);
        assert_eq!(requests[3].base_images[0].image_id, "im-layer2");
        assert!(requests[3].secret_ids.is_empty());
        assert!(requests[3].gpu_config.is_none());
        assert!(requests[3].force_build);
    }

    #[test]
    fn test_build_multi_layer_with_pending() {
        let svc = make_build_service(vec![
            // Layer 0: immediate success
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-base".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
            // Layer 1: pending, then success via streaming
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-layer1".to_string(),
                status: ImageBuildStatus::Pending,
                exception: None,
            })),
            MockBuildResponse::JoinStreaming(Ok(ImageJoinStreamingResult {
                result: Some(ImageBuildResult {
                    image_id: "im-layer1".to_string(),
                    status: ImageBuildStatus::Success,
                    exception: None,
                }),
                last_entry_id: "e1".to_string(),
            })),
        ]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "python:3.12".to_string(),
            layers: vec![
                Layer::default(),
                Layer {
                    commands: vec!["RUN pip install torch".to_string()],
                    ..Default::default()
                },
            ],
        };

        let built = svc.build(&image, &ImageBuildParams::default()).unwrap();
        assert_eq!(built.image_id, "im-layer1");
    }

    #[test]
    fn test_build_layer_failure_stops_early() {
        let svc = make_build_service(vec![
            // Layer 0: success
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-base".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
            // Layer 1: failure - should stop here
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-fail-layer".to_string(),
                status: ImageBuildStatus::Failure,
                exception: Some("compilation error".to_string()),
            })),
        ]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "python:3.12".to_string(),
            layers: vec![
                Layer::default(),
                Layer {
                    commands: vec!["RUN bad-command".to_string()],
                    ..Default::default()
                },
                Layer {
                    commands: vec!["RUN echo never-reached".to_string()],
                    ..Default::default()
                },
            ],
        };

        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("compilation error"));

        // Only 2 requests made (layer 0 and layer 1), layer 2 was never reached
        let requests = svc.client.recorded_requests.lock().unwrap();
        assert_eq!(requests.len(), 2);
    }

    #[test]
    fn test_build_validates_all_layers_before_building() {
        let svc = make_build_service(vec![]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "python:3.12".to_string(),
            layers: vec![
                Layer::default(),
                Layer {
                    commands: vec!["RUN echo ok".to_string()],
                    ..Default::default()
                },
                Layer {
                    commands: vec!["COPY . /app".to_string()],
                    ..Default::default()
                },
            ],
        };

        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("COPY"));

        // No requests should have been made
        let requests = svc.client.recorded_requests.lock().unwrap();
        assert_eq!(requests.len(), 0);
    }

    #[test]
    fn test_build_with_gpu_config() {
        let svc = make_build_service(vec![
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-base".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-gpu".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
        ]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "nvidia/cuda:12.0".to_string(),
            layers: vec![
                Layer::default(),
                Layer {
                    commands: vec!["RUN pip install torch".to_string()],
                    gpu: "A100-80GB:4".to_string(),
                    ..Default::default()
                },
            ],
        };

        let built = svc.build(&image, &ImageBuildParams::default()).unwrap();
        assert_eq!(built.image_id, "im-gpu");

        let requests = svc.client.recorded_requests.lock().unwrap();
        assert!(requests[0].gpu_config.is_none());
        let gpu = requests[1].gpu_config.as_ref().unwrap();
        assert_eq!(gpu.gpu_type, "A100-80GB");
        assert_eq!(gpu.count, 4);
    }

    #[test]
    fn test_build_invalid_gpu_config() {
        let svc = make_build_service(vec![
            MockBuildResponse::GetOrCreate(Ok(ImageBuildResult {
                image_id: "im-base".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            })),
        ]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "python:3.12".to_string(),
            layers: vec![
                Layer::default(),
                Layer {
                    commands: vec!["RUN echo hi".to_string()],
                    gpu: "T4:invalid".to_string(),
                    ..Default::default()
                },
            ],
        };

        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("invalid GPU count"));
    }

    #[test]
    fn test_build_timeout_result() {
        let svc = make_build_service(vec![MockBuildResponse::GetOrCreate(Ok(
            ImageBuildResult {
                image_id: "im-timeout".to_string(),
                status: ImageBuildStatus::Timeout,
                exception: None,
            },
        ))]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("timed out"));
    }

    #[test]
    fn test_build_terminated_result() {
        let svc = make_build_service(vec![MockBuildResponse::GetOrCreate(Ok(
            ImageBuildResult {
                image_id: "im-terminated".to_string(),
                status: ImageBuildStatus::Terminated,
                exception: None,
            },
        ))]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let err = svc
            .build(&image, &ImageBuildParams::default())
            .unwrap_err();
        assert!(err.to_string().contains("terminated"));
    }

    #[test]
    fn test_build_returns_image_with_metadata() {
        let svc = make_build_service(vec![MockBuildResponse::GetOrCreate(Ok(
            ImageBuildResult {
                image_id: "im-built".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            },
        ))]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: Some(ImageRegistryConfig {
                registry_auth_type: RegistryAuthType::Aws,
                secret_id: "secret-123".to_string(),
            }),
            tag: "my-registry/image:v1".to_string(),
            layers: vec![Layer::default()],
        };
        let built = svc.build(&image, &ImageBuildParams::default()).unwrap();
        assert_eq!(built.image_id, "im-built");
        assert_eq!(built.tag, "my-registry/image:v1");
        assert!(built.image_registry_config.is_some());
    }

    #[test]
    fn test_build_default_builder_version() {
        let svc = make_build_service(vec![MockBuildResponse::GetOrCreate(Ok(
            ImageBuildResult {
                image_id: "im-version".to_string(),
                status: ImageBuildStatus::Success,
                exception: None,
            },
        ))]);

        let image = Image {
            image_id: String::new(),
            image_registry_config: None,
            tag: "alpine:3.21".to_string(),
            layers: vec![Layer::default()],
        };
        let _ = svc.build(&image, &ImageBuildParams::default()).unwrap();

        let requests = svc.client.recorded_requests.lock().unwrap();
        // Builder version is passed through to the request but not recorded in our mock
        // The test verifies the build succeeds with default params
        assert_eq!(requests.len(), 1);
    }
}
