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

/// ImageService provides Image related operations.
pub trait ImageService: Send + Sync {
    fn from_registry(&self, tag: &str, params: Option<&ImageFromRegistryParams>) -> Image;
    fn from_aws_ecr(&self, tag: &str, secret: &Secret) -> Image;
    fn from_gcp_artifact_registry(&self, tag: &str, secret: &Secret) -> Image;
    fn from_id(&self, image_id: &str) -> Result<Image, ModalError>;
    fn delete(&self, image_id: &str, params: Option<&ImageDeleteParams>) -> Result<(), ModalError>;
}

/// Trait abstracting the gRPC calls needed for Image operations.
pub trait ImageGrpcClient: Send + Sync {
    fn image_from_id(&self, image_id: &str) -> Result<String, ModalError>;
    fn image_delete(&self, image_id: &str) -> Result<(), ModalError>;
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

    struct MockImageGrpcClient {
        from_id_result: Result<String, ModalError>,
        delete_result: Result<(), ModalError>,
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
    }

    fn make_service(
        from_id_result: Result<String, ModalError>,
        delete_result: Result<(), ModalError>,
    ) -> ImageServiceImpl<MockImageGrpcClient> {
        ImageServiceImpl {
            client: MockImageGrpcClient {
                from_id_result,
                delete_result,
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
}
