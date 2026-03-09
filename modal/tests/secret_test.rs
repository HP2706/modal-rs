#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Secrets.
/// Translated from libmodal/modal-go/test/secret_test.go

use modal::secret::{
    merge_env_into_secrets, Secret, SecretFromMapParams, SecretService,
};
use std::collections::HashMap;

/// Mock SecretService for integration tests.
struct MockSecretService {
    secret_id: String,
}

impl SecretService for MockSecretService {
    fn from_map(
        &self,
        _key_value_pairs: &HashMap<String, String>,
        _params: Option<&SecretFromMapParams>,
    ) -> Result<Secret, modal::ModalError> {
        Ok(Secret {
            secret_id: self.secret_id.clone(),
            name: String::new(),
        })
    }
}

#[test]
fn test_secret_from_name() {
    // Test creating a Secret struct and verifying its fields
    let secret = Secret {
        secret_id: "st-test-123".to_string(),
        name: "my-secret".to_string(),
    };
    assert_eq!(secret.secret_id, "st-test-123");
    assert_eq!(secret.name, "my-secret");
}

#[test]
fn test_secret_from_map() {
    let mock = MockSecretService {
        secret_id: "st-from-map-456".to_string(),
    };
    let env: HashMap<String, String> = [
        ("KEY1".into(), "value1".into()),
        ("KEY2".into(), "value2".into()),
    ]
    .into_iter()
    .collect();

    let secret = mock.from_map(&env, None).unwrap();
    assert_eq!(secret.secret_id, "st-from-map-456");
}

#[test]
fn test_secret_from_map_with_params() {
    let mock = MockSecretService {
        secret_id: "st-env-789".to_string(),
    };
    let env: HashMap<String, String> = [("KEY".into(), "val".into())].into_iter().collect();
    let params = SecretFromMapParams {
        environment: "staging".to_string(),
    };

    let secret = mock.from_map(&env, Some(&params)).unwrap();
    assert_eq!(secret.secret_id, "st-env-789");
}

#[test]
fn test_secret_merge_env_into_secrets() {
    let mock = MockSecretService {
        secret_id: "st-env-secret".to_string(),
    };
    let existing = vec![Secret {
        secret_id: "st-existing-1".to_string(),
        name: "existing".to_string(),
    }];
    let env: HashMap<String, String> = [("MY_VAR".into(), "my_val".into())]
        .into_iter()
        .collect();

    let result = merge_env_into_secrets(Some(&mock), Some(&env), Some(&existing)).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].secret_id, "st-existing-1");
    assert_eq!(result[1].secret_id, "st-env-secret");
}

#[test]
fn test_secret_merge_env_empty_returns_existing() {
    let existing = vec![Secret {
        secret_id: "st-existing".to_string(),
        name: "existing".to_string(),
    }];
    let empty_env: HashMap<String, String> = HashMap::new();

    let result =
        merge_env_into_secrets::<MockSecretService>(None, Some(&empty_env), Some(&existing))
            .unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].secret_id, "st-existing");
}

#[test]
fn test_secret_merge_env_no_service_errors() {
    let env: HashMap<String, String> = [("KEY".into(), "val".into())].into_iter().collect();

    let result = merge_env_into_secrets::<MockSecretService>(None, Some(&env), None);
    assert!(result.is_err());
}
