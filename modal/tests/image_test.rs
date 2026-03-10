#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Images.
/// Translated from libmodal/modal-go/test/image_test.go

use modal::error::ModalError;
use modal::image::{
    validate_dockerfile_commands, ImageBuildParams, ImageBuildResult, ImageBuildStatus,
    ImageDockerfileCommandsParams, ImageFromRegistryParams, ImageGrpcClient,
    ImageJoinStreamingResult, ImageLayerBuildRequest, ImageService, ImageServiceImpl,
    RegistryAuthType,
};
use modal::secret::Secret;
use std::sync::Mutex;

struct MockImageGrpcClient {
    from_id_responses: Mutex<Vec<Result<String, ModalError>>>,
    delete_responses: Mutex<Vec<Result<(), ModalError>>>,
    get_or_create_responses: Mutex<Vec<Result<ImageBuildResult, ModalError>>>,
    join_responses: Mutex<Vec<Result<ImageJoinStreamingResult, ModalError>>>,
}

impl MockImageGrpcClient {
    fn new() -> Self {
        Self {
            from_id_responses: Mutex::new(Vec::new()),
            delete_responses: Mutex::new(Vec::new()),
            get_or_create_responses: Mutex::new(Vec::new()),
            join_responses: Mutex::new(Vec::new()),
        }
    }
}

impl ImageGrpcClient for MockImageGrpcClient {
    fn image_from_id(&self, _image_id: &str) -> Result<String, ModalError> {
        self.from_id_responses.lock().unwrap().remove(0)
    }

    fn image_delete(&self, _image_id: &str) -> Result<(), ModalError> {
        self.delete_responses.lock().unwrap().remove(0)
    }

    fn image_get_or_create(
        &self,
        _request: &ImageLayerBuildRequest,
    ) -> Result<ImageBuildResult, ModalError> {
        self.get_or_create_responses.lock().unwrap().remove(0)
    }

    fn image_join_streaming(
        &self,
        _image_id: &str,
        _last_entry_id: &str,
    ) -> Result<ImageJoinStreamingResult, ModalError> {
        self.join_responses.lock().unwrap().remove(0)
    }
}

fn make_service(mock: MockImageGrpcClient) -> ImageServiceImpl<MockImageGrpcClient> {
    ImageServiceImpl { client: mock }
}

#[test]
fn test_image_from_registry() {
    let svc = make_service(MockImageGrpcClient::new());

    let image = svc.from_registry("python:3.11-slim", None);
    assert_eq!(image.tag, "python:3.11-slim");
    assert!(image.image_id.is_empty());
    assert!(image.image_registry_config.is_none());
}

#[test]
fn test_image_from_registry_with_secret() {
    let svc = make_service(MockImageGrpcClient::new());
    let secret = Secret {
        secret_id: "st-secret-123".to_string(),
        name: "my-registry-secret".to_string(),
    };

    let image = svc.from_registry(
        "my-private-registry.com/app:latest",
        Some(&ImageFromRegistryParams {
            secret: Some(secret),
        }),
    );

    assert_eq!(image.tag, "my-private-registry.com/app:latest");
    let config = image.image_registry_config.unwrap();
    assert_eq!(config.registry_auth_type, RegistryAuthType::StaticCreds);
    assert_eq!(config.secret_id, "st-secret-123");
}

#[test]
fn test_image_from_aws_ecr() {
    let svc = make_service(MockImageGrpcClient::new());
    let secret = Secret {
        secret_id: "st-aws-123".to_string(),
        name: "aws-creds".to_string(),
    };

    let image = svc.from_aws_ecr(
        "123456789.dkr.ecr.us-east-1.amazonaws.com/app:v1",
        &secret,
    );

    let config = image.image_registry_config.unwrap();
    assert_eq!(config.registry_auth_type, RegistryAuthType::Aws);
    assert_eq!(config.secret_id, "st-aws-123");
}

#[test]
fn test_image_from_gcp_artifact_registry() {
    let svc = make_service(MockImageGrpcClient::new());
    let secret = Secret {
        secret_id: "st-gcp-456".to_string(),
        name: "gcp-creds".to_string(),
    };

    let image = svc.from_gcp_artifact_registry(
        "us-docker.pkg.dev/my-project/repo/image:v1",
        &secret,
    );

    let config = image.image_registry_config.unwrap();
    assert_eq!(config.registry_auth_type, RegistryAuthType::Gcp);
    assert_eq!(config.secret_id, "st-gcp-456");
}

#[test]
fn test_image_dockerfile_commands() {
    let svc = make_service(MockImageGrpcClient::new());
    let base = svc.from_registry("python:3.11-slim", None);

    let cmds = vec![
        "RUN pip install numpy".to_string(),
        "RUN pip install pandas".to_string(),
    ];
    let extended = base.dockerfile_commands(&cmds, None);

    assert_eq!(extended.layers.len(), 2);
    assert_eq!(extended.layers[1].commands, cmds);
    assert!(extended.image_id.is_empty());
    assert_eq!(extended.tag, "python:3.11-slim");
}

#[test]
fn test_image_dockerfile_commands_chaining() {
    let svc = make_service(MockImageGrpcClient::new());

    let image = svc
        .from_registry("python:3.11-slim", None)
        .dockerfile_commands(&["RUN apt-get update".to_string()], None)
        .dockerfile_commands(&["RUN pip install torch".to_string()], None);

    assert_eq!(image.layers.len(), 3);
    assert_eq!(image.layers[1].commands, vec!["RUN apt-get update"]);
    assert_eq!(image.layers[2].commands, vec!["RUN pip install torch"]);
}

#[test]
fn test_image_build() {
    let mock = MockImageGrpcClient::new();
    mock.get_or_create_responses.lock().unwrap().push(Ok(ImageBuildResult {
        image_id: "im-built-123".to_string(),
        status: ImageBuildStatus::Success,
        exception: None,
    }));

    let svc = make_service(mock);
    let base = svc.from_registry("python:3.11-slim", None);

    let params = ImageBuildParams {
        app_id: "ap-test".to_string(),
        builder_version: "2024.10".to_string(),
    };

    let built = svc.build(&base, &params).unwrap();
    assert_eq!(built.image_id, "im-built-123");
}

#[test]
fn test_image_gpu_config() {
    let svc = make_service(MockImageGrpcClient::new());
    let base = svc.from_registry("python:3.11-slim", None);

    let params = ImageDockerfileCommandsParams {
        gpu: "T4:2".to_string(),
        ..Default::default()
    };

    let image = base.dockerfile_commands(&["RUN echo gpu".to_string()], Some(&params));
    assert_eq!(image.layers[1].gpu, "T4:2");
}

#[test]
fn test_image_dockerfile_commands_with_secrets() {
    let svc = make_service(MockImageGrpcClient::new());
    let base = svc.from_registry("python:3.11-slim", None);

    let secret = Secret {
        secret_id: "st-build-secret".to_string(),
        name: "build-secret".to_string(),
    };
    let params = ImageDockerfileCommandsParams {
        secrets: vec![secret],
        force_build: true,
        ..Default::default()
    };

    let image = base.dockerfile_commands(&["RUN train.py".to_string()], Some(&params));
    assert_eq!(image.layers[1].secrets.len(), 1);
    assert!(image.layers[1].force_build);
}

#[test]
fn test_image_validate_dockerfile_commands() {
    assert!(validate_dockerfile_commands(&["RUN echo hello".to_string()]).is_ok());
    assert!(
        validate_dockerfile_commands(&["COPY --from=builder /app /app".to_string()]).is_ok()
    );
    assert!(validate_dockerfile_commands(&["COPY ./local/file /app".to_string()]).is_err());
}

#[test]
fn test_image_from_id() {
    let mock = MockImageGrpcClient::new();
    mock.from_id_responses
        .lock()
        .unwrap()
        .push(Ok("im-resolved-789".to_string()));
    let svc = make_service(mock);

    let image = svc.from_id("im-original-789").unwrap();
    assert_eq!(image.image_id, "im-resolved-789");
}

#[test]
fn test_image_from_id_not_found() {
    let mock = MockImageGrpcClient::new();
    mock.from_id_responses
        .lock()
        .unwrap()
        .push(Err(ModalError::Grpc(tonic::Status::not_found("not found"))));
    let svc = make_service(mock);

    let err = svc.from_id("im-missing").unwrap_err();
    assert!(matches!(err, ModalError::NotFound(_)));
}

#[test]
fn test_image_delete() {
    let mock = MockImageGrpcClient::new();
    mock.delete_responses.lock().unwrap().push(Ok(()));
    let svc = make_service(mock);

    svc.delete("im-123", None).unwrap();
}
