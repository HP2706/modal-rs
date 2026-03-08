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
    fn from_map(
        &self,
        key_value_pairs: &std::collections::HashMap<String, String>,
        params: Option<&SecretFromMapParams>,
    ) -> Result<Secret, crate::error::ModalError>;
}

/// Merge environment variables into the secrets list.
/// If env contains values, it creates a new Secret from the env map and appends it
/// to the existing secrets.
pub fn merge_env_into_secrets<S: SecretService>(
    secret_service: Option<&S>,
    env: Option<&std::collections::HashMap<String, String>>,
    secrets: Option<&[Secret]>,
) -> Result<Vec<Secret>, crate::error::ModalError> {
    let mut result: Vec<Secret> = Vec::new();

    if let Some(s) = secrets {
        result.extend(s.iter().cloned());
    }

    if let Some(e) = env {
        if !e.is_empty() {
            let svc = secret_service
                .ok_or_else(|| crate::error::ModalError::Other("secret service required".to_string()))?;
            let env_secret = svc.from_map(e, None)?;
            result.push(env_secret);
        }
    }

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    struct MockSecretService;

    impl SecretService for MockSecretService {
        fn from_map(
            &self,
            _key_value_pairs: &HashMap<String, String>,
            _params: Option<&SecretFromMapParams>,
        ) -> Result<Secret, crate::error::ModalError> {
            Ok(Secret {
                secret_id: "st-mock-env".to_string(),
                name: String::new(),
            })
        }
    }

    #[test]
    fn test_merge_env_into_secrets_with_env_and_existing_secrets() {
        let mock = MockSecretService;
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
        let mock = MockSecretService;
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
            merge_env_into_secrets::<MockSecretService>(None, Some(&env), Some(&existing)).unwrap();
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
            merge_env_into_secrets::<MockSecretService>(None, None, Some(&existing)).unwrap();
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
            merge_env_into_secrets::<MockSecretService>(None, None, Some(&secrets)).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].secret_id, "st-secret1");
        assert_eq!(result[1].secret_id, "st-secret2");
    }

    #[test]
    fn test_merge_env_into_secrets_with_no_env_and_no_secrets() {
        let result = merge_env_into_secrets::<MockSecretService>(None, None, None).unwrap();
        assert_eq!(result.len(), 0);
    }
}
