use crate::config::{environment_name, Profile};
use crate::error::ModalError;

/// Proxy represents a Modal Proxy.
#[derive(Debug, Clone)]
pub struct Proxy {
    pub proxy_id: String,
}

/// ProxyFromNameParams are options for looking up a Modal Proxy.
#[derive(Debug, Clone, Default)]
pub struct ProxyFromNameParams {
    pub environment: String,
}

/// ProxyService provides Proxy related operations.
pub trait ProxyService: Send + Sync {
    fn from_name(
        &self,
        name: &str,
        params: Option<&ProxyFromNameParams>,
    ) -> Result<Proxy, ModalError>;
}

/// Trait abstracting the gRPC calls needed by ProxyServiceImpl.
pub trait ProxyGrpcClient: Send + Sync {
    /// Calls ProxyGet and returns the proxy_id if found.
    fn proxy_get(
        &self,
        name: &str,
        environment_name: &str,
    ) -> Result<Option<String>, ModalError>;
}

/// Implementation of ProxyService backed by a gRPC client.
pub struct ProxyServiceImpl<C: ProxyGrpcClient> {
    pub client: C,
    pub profile: Profile,
}

impl<C: ProxyGrpcClient> ProxyService for ProxyServiceImpl<C> {
    fn from_name(
        &self,
        name: &str,
        params: Option<&ProxyFromNameParams>,
    ) -> Result<Proxy, ModalError> {
        let default_params = ProxyFromNameParams::default();
        let params = params.unwrap_or(&default_params);

        let env = environment_name(&params.environment, &self.profile);

        let proxy_id = self.client.proxy_get(name, &env).map_err(|e| {
            if matches!(&e, ModalError::Grpc(s) if s.code() == tonic::Code::NotFound) {
                ModalError::NotFound(format!("Proxy '{}' not found", name))
            } else {
                e
            }
        })?;

        match proxy_id {
            Some(id) if !id.is_empty() => Ok(Proxy { proxy_id: id }),
            _ => Err(ModalError::NotFound(format!(
                "Proxy '{}' not found",
                name
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockProxyGrpcClient {
        result: Result<Option<String>, ModalError>,
    }

    impl ProxyGrpcClient for MockProxyGrpcClient {
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

    fn make_service(mock: MockProxyGrpcClient) -> ProxyServiceImpl<MockProxyGrpcClient> {
        ProxyServiceImpl {
            client: mock,
            profile: Profile::default(),
        }
    }

    #[test]
    fn test_proxy_from_name_success() {
        let svc = make_service(MockProxyGrpcClient {
            result: Ok(Some("pr-test-123".to_string())),
        });

        let proxy = svc.from_name("my-proxy", None).unwrap();
        assert_eq!(proxy.proxy_id, "pr-test-123");
    }

    #[test]
    fn test_proxy_from_name_not_found_empty_id() {
        let svc = make_service(MockProxyGrpcClient {
            result: Ok(Some(String::new())),
        });

        let err = svc.from_name("my-proxy", None).unwrap_err();
        assert!(
            err.to_string().contains("Proxy 'my-proxy' not found"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_proxy_from_name_not_found_none() {
        let svc = make_service(MockProxyGrpcClient { result: Ok(None) });

        let err = svc.from_name("my-proxy", None).unwrap_err();
        assert!(
            err.to_string().contains("Proxy 'my-proxy' not found"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_proxy_from_name_with_params() {
        let svc = make_service(MockProxyGrpcClient {
            result: Ok(Some("pr-test-456".to_string())),
        });

        let proxy = svc
            .from_name(
                "my-proxy",
                Some(&ProxyFromNameParams {
                    environment: "staging".to_string(),
                }),
            )
            .unwrap();
        assert_eq!(proxy.proxy_id, "pr-test-456");
    }
}
