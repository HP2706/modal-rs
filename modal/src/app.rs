use crate::config::{environment_name, Profile};
use crate::error::ModalError;

/// App references a deployed Modal App.
#[derive(Debug, Clone)]
pub struct App {
    pub app_id: String,
    pub name: String,
}

/// AppFromNameParams are options for client.Apps.FromName.
#[derive(Debug, Clone, Default)]
pub struct AppFromNameParams {
    pub environment: String,
    pub create_if_missing: bool,
}

/// AppService provides App related operations.
pub trait AppService: Send + Sync {
    fn from_name(
        &self,
        name: &str,
        params: Option<&AppFromNameParams>,
    ) -> Result<App, ModalError>;
}

/// Trait abstracting the gRPC calls needed by AppServiceImpl.
pub trait AppGrpcClient: Send + Sync {
    fn app_get_or_create(
        &self,
        app_name: &str,
        environment_name: &str,
        object_creation_type: i32,
    ) -> Result<String, ModalError>;
}

/// Implementation of AppService backed by a gRPC client.
pub struct AppServiceImpl<C: AppGrpcClient> {
    pub client: C,
    pub profile: Profile,
}

impl<C: AppGrpcClient> AppService for AppServiceImpl<C> {
    fn from_name(
        &self,
        name: &str,
        params: Option<&AppFromNameParams>,
    ) -> Result<App, ModalError> {
        let default_params = AppFromNameParams::default();
        let params = params.unwrap_or(&default_params);

        let creation_type = if params.create_if_missing { 1 } else { 0 };
        let env = environment_name(&params.environment, &self.profile);

        let app_id = self
            .client
            .app_get_or_create(name, &env, creation_type)
            .map_err(|e| {
                if matches!(&e, ModalError::Grpc(s) if s.code() == tonic::Code::NotFound) {
                    ModalError::NotFound(format!("App '{}' not found", name))
                } else {
                    e
                }
            })?;

        Ok(App {
            app_id,
            name: name.to_string(),
        })
    }
}

/// GPUConfig parsed from a GPU string.
#[derive(Debug, Clone)]
pub struct GpuConfig {
    pub gpu_type: String,
    pub count: u32,
}

/// Parse a GPU configuration string into a GpuConfig.
/// The GPU string format is "type" or "type:count" (e.g. "T4", "A100:2").
/// Returns a config with empty type and count=0 if gpu is empty.
pub fn parse_gpu_config(gpu: &str) -> Result<GpuConfig, ModalError> {
    if gpu.is_empty() {
        return Ok(GpuConfig {
            gpu_type: String::new(),
            count: 0,
        });
    }

    let (gpu_type, count) = if let Some(idx) = gpu.find(':') {
        let type_part = &gpu[..idx];
        let count_str = &gpu[idx + 1..];
        let parsed_count: u64 = count_str.parse().map_err(|_| {
            ModalError::Invalid(format!(
                "invalid GPU count: {}, value must be a positive integer",
                count_str
            ))
        })?;
        if parsed_count < 1 {
            return Err(ModalError::Invalid(format!(
                "invalid GPU count: {}, value must be a positive integer",
                count_str
            )));
        }
        (type_part.to_uppercase(), parsed_count as u32)
    } else {
        (gpu.to_uppercase(), 1)
    };

    Ok(GpuConfig { gpu_type, count })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockAppGrpcClient {
        result: Result<String, ModalError>,
    }

    impl AppGrpcClient for MockAppGrpcClient {
        fn app_get_or_create(
            &self,
            _app_name: &str,
            _environment_name: &str,
            _object_creation_type: i32,
        ) -> Result<String, ModalError> {
            match &self.result {
                Ok(v) => Ok(v.clone()),
                Err(e) => Err(ModalError::Other(e.to_string())),
            }
        }
    }

    fn make_service(mock: MockAppGrpcClient) -> AppServiceImpl<MockAppGrpcClient> {
        AppServiceImpl {
            client: mock,
            profile: Profile::default(),
        }
    }

    #[test]
    fn test_app_from_name_success() {
        let svc = make_service(MockAppGrpcClient {
            result: Ok("ap-test-123".to_string()),
        });
        let app = svc.from_name("my-app", None).unwrap();
        assert_eq!(app.app_id, "ap-test-123");
        assert_eq!(app.name, "my-app");
    }

    #[test]
    fn test_app_from_name_with_params() {
        let svc = make_service(MockAppGrpcClient {
            result: Ok("ap-test-456".to_string()),
        });
        let params = AppFromNameParams {
            environment: "staging".to_string(),
            create_if_missing: true,
        };
        let app = svc.from_name("my-app", Some(&params)).unwrap();
        assert_eq!(app.app_id, "ap-test-456");
    }

    #[test]
    fn test_app_from_name_error() {
        let svc = make_service(MockAppGrpcClient {
            result: Err(ModalError::Other("connection failed".to_string())),
        });
        let err = svc.from_name("my-app", None).unwrap_err();
        assert!(err.to_string().contains("connection failed"), "got: {}", err);
    }

    #[test]
    fn test_parse_gpu_config() {
        // Empty
        let config = parse_gpu_config("").unwrap();
        assert_eq!(config.count, 0);
        assert_eq!(config.gpu_type, "");

        // Simple type
        let config = parse_gpu_config("T4").unwrap();
        assert_eq!(config.count, 1);
        assert_eq!(config.gpu_type, "T4");

        let config = parse_gpu_config("A10G").unwrap();
        assert_eq!(config.count, 1);
        assert_eq!(config.gpu_type, "A10G");

        let config = parse_gpu_config("A100-80GB").unwrap();
        assert_eq!(config.count, 1);
        assert_eq!(config.gpu_type, "A100-80GB");

        // Type with count
        let config = parse_gpu_config("A100-80GB:3").unwrap();
        assert_eq!(config.count, 3);
        assert_eq!(config.gpu_type, "A100-80GB");

        let config = parse_gpu_config("T4:2").unwrap();
        assert_eq!(config.count, 2);
        assert_eq!(config.gpu_type, "T4");

        // Case insensitive
        let config = parse_gpu_config("a100:4").unwrap();
        assert_eq!(config.count, 4);
        assert_eq!(config.gpu_type, "A100");

        // Error cases
        let err = parse_gpu_config("T4:invalid").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: invalid"), "got: {}", err);

        let err = parse_gpu_config("T4:").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: "), "got: {}", err);

        let err = parse_gpu_config("T4:0").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: 0"), "got: {}", err);

        let err = parse_gpu_config("T4:-1").unwrap_err().to_string();
        assert!(err.contains("invalid GPU count: -1"), "got: {}", err);
    }
}
