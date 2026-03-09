#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Proxies.
/// Translated from libmodal/modal-go/test/proxy_test.go

use modal::error::ModalError;
use modal::proxy::{ProxyFromNameParams, ProxyGrpcClient, ProxyService, ProxyServiceImpl};

struct MockProxyClient {
    result: Result<Option<String>, ModalError>,
}

impl ProxyGrpcClient for MockProxyClient {
    fn proxy_get(
        &self,
        _name: &str,
        _environment_name: &str,
    ) -> Result<Option<String>, ModalError> {
        match &self.result {
            Ok(v) => Ok(v.clone()),
            Err(e) => Err(ModalError::Other(e.to_string())),
        }
    }
}

fn make_service(mock: MockProxyClient) -> ProxyServiceImpl<MockProxyClient> {
    ProxyServiceImpl {
        client: mock,
        profile: modal::config::Profile::default(),
    }
}

#[test]
fn test_proxy_create() {
    let svc = make_service(MockProxyClient {
        result: Ok(Some("pr-test-123".to_string())),
    });

    let proxy = svc.from_name("my-proxy", None).unwrap();
    assert_eq!(proxy.proxy_id, "pr-test-123");
}

#[test]
fn test_proxy_with_environment() {
    let svc = make_service(MockProxyClient {
        result: Ok(Some("pr-staging-456".to_string())),
    });

    let proxy = svc
        .from_name(
            "my-proxy",
            Some(&ProxyFromNameParams {
                environment: "staging".to_string(),
            }),
        )
        .unwrap();
    assert_eq!(proxy.proxy_id, "pr-staging-456");
}

#[test]
fn test_proxy_not_found() {
    let svc = make_service(MockProxyClient {
        result: Ok(None),
    });

    let err = svc.from_name("missing-proxy", None).unwrap_err();
    assert!(err.to_string().contains("not found"), "got: {}", err);
}

#[test]
fn test_proxy_not_found_empty_id() {
    let svc = make_service(MockProxyClient {
        result: Ok(Some(String::new())),
    });

    let err = svc.from_name("empty-proxy", None).unwrap_err();
    assert!(err.to_string().contains("not found"), "got: {}", err);
}
