use std::collections::HashMap;

use crate::config::{environment_name, Profile};
use crate::error::ModalError;

/// Secret represents a Modal Secret.
#[derive(Debug, Clone)]
pub struct Secret {
    pub secret_id: String,
    pub name: String,
}

/// SecretFromNameParams are options for finding Modal Secrets.
#[derive(Debug, Clone, Default)]
pub struct SecretFromNameParams {
    pub environment: String,
    pub required_keys: Vec<String>,
}

/// SecretFromMapParams are options for creating a Secret from a key/value map.
#[derive(Debug, Clone, Default)]
pub struct SecretFromMapParams {
    pub environment: String,
}

/// SecretDeleteParams are options for deleting a Secret.
#[derive(Debug, Clone, Default)]
pub struct SecretDeleteParams {
    pub environment: String,
    pub allow_missing: bool,
}

/// SecretService trait for secret operations.
pub trait SecretService: Send + Sync {
    fn from_name(
        &self,
        name: &str,
        params: Option<&SecretFromNameParams>,
    ) -> Result<Secret, ModalError>;

    fn from_map(
        &self,
        key_value_pairs: &HashMap<String, String>,
        params: Option<&SecretFromMapParams>,
    ) -> Result<Secret, ModalError>;

    fn delete(&self, name: &str, params: Option<&SecretDeleteParams>) -> Result<(), ModalError>;
}

/// Trait abstracting the gRPC calls needed by SecretServiceImpl.
pub trait SecretGrpcClient: Send + Sync {
    fn secret_get_or_create(
        &self,
        deployment_name: &str,
        environment_name: &str,
        required_keys: &[String],
        object_creation_type: i32,
        env_dict: &HashMap<String, String>,
    ) -> Result<String, ModalError>;

    fn secret_delete(&self, secret_id: &str) -> Result<(), ModalError>;
}

/// Implementation of SecretService backed by a gRPC client.
pub struct SecretServiceImpl<C: SecretGrpcClient> {
    pub client: C,
    pub profile: Profile,
}

impl<C: SecretGrpcClient> SecretService for SecretServiceImpl<C> {
    fn from_name(
        &self,
        name: &str,
        params: Option<&SecretFromNameParams>,
    ) -> Result<Secret, ModalError> {
        let default_params = SecretFromNameParams::default();
        let params = params.unwrap_or(&default_params);

        let env = environment_name(&params.environment, &self.profile);

        let secret_id = self
            .client
            .secret_get_or_create(
                name,
                &env,
                &params.required_keys,
                0, // OBJECT_CREATION_TYPE_UNSPECIFIED
                &HashMap::new(),
            )
            .map_err(|e| {
                if is_not_found_error(&e) {
                    ModalError::NotFound(format!("Secret '{}' not found", name))
                } else {
                    e
                }
            })?;

        Ok(Secret {
            secret_id,
            name: name.to_string(),
        })
    }

    fn from_map(
        &self,
        key_value_pairs: &HashMap<String, String>,
        params: Option<&SecretFromMapParams>,
    ) -> Result<Secret, ModalError> {
        let default_params = SecretFromMapParams::default();
        let params = params.unwrap_or(&default_params);

        let env = environment_name(&params.environment, &self.profile);

        let secret_id = self.client.secret_get_or_create(
            "",
            &env,
            &[],
            5, // OBJECT_CREATION_TYPE_EPHEMERAL
            key_value_pairs,
        )?;

        Ok(Secret {
            secret_id,
            name: String::new(),
        })
    }

    fn delete(&self, name: &str, params: Option<&SecretDeleteParams>) -> Result<(), ModalError> {
        let default_params = SecretDeleteParams::default();
        let params = params.unwrap_or(&default_params);

        let secret = self.from_name(
            name,
            Some(&SecretFromNameParams {
                environment: params.environment.clone(),
                ..Default::default()
            }),
        );

        let secret = match secret {
            Ok(s) => s,
            Err(e) => {
                if is_not_found_error(&e) && params.allow_missing {
                    return Ok(());
                }
                return Err(e);
            }
        };

        match self.client.secret_delete(&secret.secret_id) {
            Ok(()) => Ok(()),
            Err(e) => {
                if is_grpc_not_found(&e) && params.allow_missing {
                    Ok(())
                } else {
                    Err(e)
                }
            }
        }
    }
}

/// Check if an error is a NotFound error.
fn is_not_found_error(err: &ModalError) -> bool {
    match err {
        ModalError::NotFound(_) => true,
        ModalError::Grpc(status) => status.code() == tonic::Code::NotFound,
        _ => false,
    }
}

/// Check if an error is a gRPC NotFound status.
fn is_grpc_not_found(err: &ModalError) -> bool {
    match err {
        ModalError::Grpc(status) => status.code() == tonic::Code::NotFound,
        _ => false,
    }
}

/// Merge environment variables into the secrets list.
/// If env contains values, it creates a new Secret from the env map and appends it
/// to the existing secrets.
pub fn merge_env_into_secrets<S: SecretService>(
    secret_service: Option<&S>,
    env: Option<&HashMap<String, String>>,
    secrets: Option<&[Secret]>,
) -> Result<Vec<Secret>, ModalError> {
    let mut result: Vec<Secret> = Vec::new();

    if let Some(s) = secrets {
        result.extend(s.iter().cloned());
    }

    if let Some(e) = env {
        if !e.is_empty() {
            let svc = secret_service
                .ok_or_else(|| ModalError::Other("secret service required".to_string()))?;
            let env_secret = svc.from_map(e, None)?;
            result.push(env_secret);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock gRPC client for testing secret operations.
    struct MockSecretGrpcClient {
        responses: Mutex<Vec<MockResponse>>,
    }

    enum MockResponse {
        GetOrCreate(Result<String, ModalError>),
        Delete(Result<(), ModalError>),
    }

    impl MockSecretGrpcClient {
        fn new() -> Self {
            Self {
                responses: Mutex::new(Vec::new()),
            }
        }

        fn push_get_or_create(&self, result: Result<String, ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::GetOrCreate(result));
        }

        fn push_delete(&self, result: Result<(), ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::Delete(result));
        }
    }

    impl SecretGrpcClient for MockSecretGrpcClient {
        fn secret_get_or_create(
            &self,
            _deployment_name: &str,
            _environment_name: &str,
            _required_keys: &[String],
            _object_creation_type: i32,
            _env_dict: &HashMap<String, String>,
        ) -> Result<String, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::GetOrCreate(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn secret_delete(&self, _secret_id: &str) -> Result<(), ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Delete(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }
    }

    fn make_service(mock: MockSecretGrpcClient) -> SecretServiceImpl<MockSecretGrpcClient> {
        SecretServiceImpl {
            client: mock,
            profile: Profile::default(),
        }
    }

    // --- from_name tests ---

    #[test]
    fn test_secret_from_name() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Ok("st-test-123".to_string()));
        let svc = make_service(mock);

        let secret = svc.from_name("my-secret", None).unwrap();
        assert!(secret.secret_id.starts_with("st-"));
        assert_eq!(secret.name, "my-secret");
    }

    #[test]
    fn test_secret_from_name_not_found() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::Grpc(tonic::Status::not_found(
            "not found",
        ))));
        let svc = make_service(mock);

        let err = svc.from_name("missing-secret", None).unwrap_err();
        assert!(
            err.to_string().contains("Secret 'missing-secret' not found"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_secret_from_name_with_required_keys() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Ok("st-keys-456".to_string()));
        let svc = make_service(mock);

        let secret = svc
            .from_name(
                "my-secret",
                Some(&SecretFromNameParams {
                    required_keys: vec!["a".into(), "b".into(), "c".into()],
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(secret.secret_id, "st-keys-456");
    }

    #[test]
    fn test_secret_from_name_with_environment() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Ok("st-env-789".to_string()));
        let svc = make_service(mock);

        let secret = svc
            .from_name(
                "my-secret",
                Some(&SecretFromNameParams {
                    environment: "staging".to_string(),
                    ..Default::default()
                }),
            )
            .unwrap();
        assert_eq!(secret.secret_id, "st-env-789");
    }

    // --- from_map tests ---

    #[test]
    fn test_secret_from_map() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Ok("st-map-123".to_string()));
        let svc = make_service(mock);

        let env: HashMap<String, String> =
            [("KEY".into(), "value".into())].into_iter().collect();
        let secret = svc.from_map(&env, None).unwrap();
        assert_eq!(secret.secret_id, "st-map-123");
        assert_eq!(secret.name, ""); // ephemeral secrets have no name
    }

    #[test]
    fn test_secret_from_map_with_params() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Ok("st-map-env-456".to_string()));
        let svc = make_service(mock);

        let env: HashMap<String, String> =
            [("KEY".into(), "value".into())].into_iter().collect();
        let params = SecretFromMapParams {
            environment: "prod".to_string(),
        };
        let secret = svc.from_map(&env, Some(&params)).unwrap();
        assert_eq!(secret.secret_id, "st-map-env-456");
    }

    // --- delete tests ---

    #[test]
    fn test_secret_delete_success() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Ok("st-test-123".to_string()));
        mock.push_delete(Ok(()));
        let svc = make_service(mock);

        svc.delete("test-secret", None).unwrap();
    }

    #[test]
    fn test_secret_delete_with_allow_missing() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::NotFound(
            "Secret 'missing' not found".to_string(),
        )));
        let svc = make_service(mock);

        svc.delete(
            "missing",
            Some(&SecretDeleteParams {
                allow_missing: true,
                ..Default::default()
            }),
        )
        .unwrap();
    }

    #[test]
    fn test_secret_delete_with_allow_missing_delete_rpc_not_found() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Ok("st-test-123".to_string()));
        mock.push_delete(Err(ModalError::Grpc(tonic::Status::not_found(
            "Secret not found",
        ))));
        let svc = make_service(mock);

        svc.delete(
            "test-secret",
            Some(&SecretDeleteParams {
                allow_missing: true,
                ..Default::default()
            }),
        )
        .unwrap();
    }

    #[test]
    fn test_secret_delete_with_allow_missing_false_throws() {
        let mock = MockSecretGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::NotFound(
            "Secret 'missing' not found".to_string(),
        )));
        let svc = make_service(mock);

        let err = svc
            .delete(
                "missing",
                Some(&SecretDeleteParams {
                    allow_missing: false,
                    ..Default::default()
                }),
            )
            .unwrap_err();

        assert!(matches!(err, ModalError::NotFound(_)));
    }

    // --- merge_env_into_secrets tests (using SecretServiceImpl) ---

    struct SimpleMockSecretService;

    impl SecretService for SimpleMockSecretService {
        fn from_name(
            &self,
            name: &str,
            _params: Option<&SecretFromNameParams>,
        ) -> Result<Secret, ModalError> {
            Ok(Secret {
                secret_id: format!("st-{}", name),
                name: name.to_string(),
            })
        }

        fn from_map(
            &self,
            _key_value_pairs: &HashMap<String, String>,
            _params: Option<&SecretFromMapParams>,
        ) -> Result<Secret, ModalError> {
            Ok(Secret {
                secret_id: "st-mock-env".to_string(),
                name: String::new(),
            })
        }

        fn delete(
            &self,
            _name: &str,
            _params: Option<&SecretDeleteParams>,
        ) -> Result<(), ModalError> {
            Ok(())
        }
    }

    #[test]
    fn test_merge_env_into_secrets_with_env_and_existing_secrets() {
        let mock = SimpleMockSecretService;
        let env: HashMap<String, String> =
            [("B".into(), "2".into()), ("C".into(), "3".into())]
                .into_iter()
                .collect();
        let existing = vec![Secret {
            secret_id: "st-existing".to_string(),
            name: String::new(),
        }];

        let result = merge_env_into_secrets(Some(&mock), Some(&env), Some(&existing)).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].secret_id, "st-existing");
        assert_eq!(result[1].secret_id, "st-mock-env");
    }

    #[test]
    fn test_merge_env_into_secrets_with_only_env() {
        let mock = SimpleMockSecretService;
        let env: HashMap<String, String> =
            [("B".into(), "2".into()), ("C".into(), "3".into())]
                .into_iter()
                .collect();

        let result = merge_env_into_secrets(Some(&mock), Some(&env), None).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].secret_id, "st-mock-env");
    }

    #[test]
    fn test_merge_env_into_secrets_with_empty_env_returns_existing() {
        let existing = vec![Secret {
            secret_id: "st-existing".to_string(),
            name: String::new(),
        }];
        let env: HashMap<String, String> = HashMap::new();

        let result =
            merge_env_into_secrets::<SimpleMockSecretService>(None, Some(&env), Some(&existing))
                .unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].secret_id, "st-existing");
    }

    #[test]
    fn test_merge_env_into_secrets_with_nil_env_returns_existing() {
        let existing = vec![Secret {
            secret_id: "st-existing".to_string(),
            name: String::new(),
        }];

        let result =
            merge_env_into_secrets::<SimpleMockSecretService>(None, None, Some(&existing)).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].secret_id, "st-existing");
    }

    #[test]
    fn test_merge_env_into_secrets_with_only_existing_secrets() {
        let s1 = Secret {
            secret_id: "st-secret1".to_string(),
            name: String::new(),
        };
        let s2 = Secret {
            secret_id: "st-secret2".to_string(),
            name: String::new(),
        };
        let secrets = vec![s1, s2];

        let result =
            merge_env_into_secrets::<SimpleMockSecretService>(None, None, Some(&secrets)).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].secret_id, "st-secret1");
        assert_eq!(result[1].secret_id, "st-secret2");
    }

    #[test]
    fn test_merge_env_into_secrets_with_no_env_and_no_secrets() {
        let result = merge_env_into_secrets::<SimpleMockSecretService>(None, None, None).unwrap();
        assert_eq!(result.len(), 0);
    }

    #[test]
    fn test_is_not_found_error() {
        assert!(is_not_found_error(&ModalError::NotFound("test".to_string())));
        assert!(is_not_found_error(&ModalError::Grpc(
            tonic::Status::not_found("test")
        )));
        assert!(!is_not_found_error(&ModalError::Other("test".to_string())));
    }
}
