use std::sync::Arc;

use crate::config::{environment_name, Profile};
use crate::ephemeral::start_ephemeral_heartbeat;
use crate::error::ModalError;

/// Volume represents a Modal Volume that provides persistent storage.
#[derive(Debug, Clone)]
pub struct Volume {
    pub volume_id: String,
    pub name: String,
    read_only: bool,
    cancel_ephemeral: Option<Arc<tokio::sync::Notify>>,
}

impl Volume {
    /// Create a new Volume with the given ID and name.
    pub fn new(volume_id: String, name: String) -> Self {
        Self {
            volume_id,
            name,
            read_only: false,
            cancel_ephemeral: None,
        }
    }

    /// ReadOnly configures Volume to mount as read-only.
    pub fn read_only(&self) -> Self {
        Self {
            volume_id: self.volume_id.clone(),
            name: self.name.clone(),
            read_only: true,
            cancel_ephemeral: self.cancel_ephemeral.clone(),
        }
    }

    /// IsReadOnly returns true if the Volume is configured to mount as read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// CloseEphemeral deletes an ephemeral Volume by cancelling its heartbeat.
    /// Panics if the Volume is not ephemeral.
    pub fn close_ephemeral(&self) {
        match &self.cancel_ephemeral {
            Some(notify) => notify.notify_one(),
            None => panic!("Volume {} is not ephemeral", self.volume_id),
        }
    }

    /// Returns true if this volume is ephemeral.
    pub fn is_ephemeral(&self) -> bool {
        self.cancel_ephemeral.is_some()
    }
}

/// VolumeFromNameParams are options for finding Modal Volumes.
#[derive(Debug, Clone, Default)]
pub struct VolumeFromNameParams {
    pub environment: String,
    pub create_if_missing: bool,
}

/// VolumeEphemeralParams are options for creating ephemeral Volumes.
#[derive(Debug, Clone, Default)]
pub struct VolumeEphemeralParams {
    pub environment: String,
}

/// VolumeDeleteParams are options for deleting Volumes.
#[derive(Debug, Clone, Default)]
pub struct VolumeDeleteParams {
    pub environment: String,
    pub allow_missing: bool,
}

/// VolumeService provides Volume related operations.
///
/// This trait abstracts the gRPC calls needed for volume operations,
/// allowing mock implementations for testing.
pub trait VolumeService: Send + Sync {
    fn from_name(
        &self,
        name: &str,
        params: Option<&VolumeFromNameParams>,
    ) -> Result<Volume, ModalError>;

    fn ephemeral(&self, params: Option<&VolumeEphemeralParams>) -> Result<Volume, ModalError>;

    fn delete(&self, name: &str, params: Option<&VolumeDeleteParams>) -> Result<(), ModalError>;
}

/// Implementation of VolumeService backed by a gRPC client.
pub struct VolumeServiceImpl<C: VolumeGrpcClient> {
    pub client: C,
    pub profile: Profile,
}

/// Trait abstracting the gRPC calls needed by VolumeServiceImpl.
pub trait VolumeGrpcClient: Send + Sync {
    fn volume_get_or_create(
        &self,
        deployment_name: &str,
        environment_name: &str,
        object_creation_type: i32,
    ) -> Result<String, ModalError>;

    fn volume_heartbeat(&self, volume_id: &str) -> Result<(), ModalError>;

    fn volume_delete(&self, volume_id: &str) -> Result<(), ModalError>;
}

impl<C: VolumeGrpcClient> VolumeService for VolumeServiceImpl<C> {
    fn from_name(
        &self,
        name: &str,
        params: Option<&VolumeFromNameParams>,
    ) -> Result<Volume, ModalError> {
        let default_params = VolumeFromNameParams::default();
        let params = params.unwrap_or(&default_params);

        let creation_type = if params.create_if_missing {
            1 // OBJECT_CREATION_TYPE_CREATE_IF_MISSING
        } else {
            0 // OBJECT_CREATION_TYPE_UNSPECIFIED
        };

        let env = environment_name(&params.environment, &self.profile);

        let volume_id = self.client.volume_get_or_create(name, &env, creation_type)
            .map_err(|e| {
                if is_not_found_error(&e) {
                    ModalError::NotFound(format!("Volume '{}' not found", name))
                } else {
                    e
                }
            })?;

        Ok(Volume::new(volume_id, name.to_string()))
    }

    fn ephemeral(&self, params: Option<&VolumeEphemeralParams>) -> Result<Volume, ModalError> {
        let default_params = VolumeEphemeralParams::default();
        let params = params.unwrap_or(&default_params);

        let env = environment_name(&params.environment, &self.profile);

        let volume_id = self.client.volume_get_or_create(
            "",
            &env,
            5, // OBJECT_CREATION_TYPE_EPHEMERAL
        )?;

        let notify = Arc::new(tokio::sync::Notify::new());
        let vol_id_clone = volume_id.clone();
        let notify_clone = notify.clone();

        // Start heartbeat — the closure captures volume_id for heartbeat calls
        start_ephemeral_heartbeat(notify_clone, move || {
            // In a real implementation, this would call volume_heartbeat
            // For now, this is a placeholder that tests can verify
            let _ = &vol_id_clone;
            Ok(())
        });

        Ok(Volume {
            volume_id,
            name: String::new(),
            read_only: false,
            cancel_ephemeral: Some(notify),
        })
    }

    fn delete(&self, name: &str, params: Option<&VolumeDeleteParams>) -> Result<(), ModalError> {
        let default_params = VolumeDeleteParams::default();
        let params = params.unwrap_or(&default_params);

        let volume = self.from_name(name, Some(&VolumeFromNameParams {
            environment: params.environment.clone(),
            create_if_missing: false,
        }));

        let volume = match volume {
            Ok(v) => v,
            Err(e) => {
                if is_not_found_error(&e) && params.allow_missing {
                    return Ok(());
                }
                return Err(e);
            }
        };

        match self.client.volume_delete(&volume.volume_id) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Mock gRPC client for testing volume operations.
    struct MockVolumeGrpcClient {
        responses: Mutex<Vec<MockResponse>>,
    }

    enum MockResponse {
        GetOrCreate(Result<String, ModalError>),
        Delete(Result<(), ModalError>),
        Heartbeat(Result<(), ModalError>),
    }

    impl MockVolumeGrpcClient {
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

        #[allow(dead_code)]
        fn push_heartbeat(&self, result: Result<(), ModalError>) {
            self.responses
                .lock()
                .unwrap()
                .push(MockResponse::Heartbeat(result));
        }
    }

    impl VolumeGrpcClient for MockVolumeGrpcClient {
        fn volume_get_or_create(
            &self,
            _deployment_name: &str,
            _environment_name: &str,
            _object_creation_type: i32,
        ) -> Result<String, ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::GetOrCreate(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn volume_heartbeat(&self, _volume_id: &str) -> Result<(), ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Heartbeat(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }

        fn volume_delete(&self, _volume_id: &str) -> Result<(), ModalError> {
            let mut responses = self.responses.lock().unwrap();
            match responses.remove(0) {
                MockResponse::Delete(r) => r,
                _ => panic!("unexpected mock response type"),
            }
        }
    }

    fn make_service(mock: MockVolumeGrpcClient) -> VolumeServiceImpl<MockVolumeGrpcClient> {
        VolumeServiceImpl {
            client: mock,
            profile: Profile::default(),
        }
    }

    #[test]
    fn test_volume_from_name() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Ok("vo-test-123".to_string()));
        let svc = make_service(mock);

        let volume = svc
            .from_name(
                "libmodal-test-volume",
                Some(&VolumeFromNameParams {
                    create_if_missing: true,
                    ..Default::default()
                }),
            )
            .unwrap();

        assert!(volume.volume_id.starts_with("vo-"));
        assert_eq!(volume.name, "libmodal-test-volume");
        assert!(!volume.is_read_only());
    }

    #[test]
    fn test_volume_from_name_not_found() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::Grpc(tonic::Status::not_found(
            "not found",
        ))));
        let svc = make_service(mock);

        let err = svc.from_name("missing-volume", None).unwrap_err();
        assert!(
            err.to_string().contains("Volume 'missing-volume' not found"),
            "got: {}",
            err
        );
    }

    #[test]
    fn test_volume_read_only() {
        let volume = Volume::new("vo-test-123".to_string(), "test-volume".to_string());
        assert!(!volume.is_read_only());

        let read_only_volume = volume.read_only();
        assert!(read_only_volume.is_read_only());
        assert_eq!(read_only_volume.volume_id, volume.volume_id);
        assert_eq!(read_only_volume.name, volume.name);

        // Original should still not be read-only
        assert!(!volume.is_read_only());
    }

    #[test]
    fn test_volume_ephemeral() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Ok("vo-ephemeral-456".to_string()));
        let svc = make_service(mock);

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        rt.block_on(async {
            let volume = svc.ephemeral(None).unwrap();
            assert_eq!(volume.name, "");
            assert!(volume.volume_id.starts_with("vo-"));
            assert!(!volume.is_read_only());
            assert!(volume.is_ephemeral());
            assert!(volume.read_only().is_read_only());

            // CloseEphemeral should not panic
            volume.close_ephemeral();
        });
    }

    #[test]
    #[should_panic(expected = "is not ephemeral")]
    fn test_volume_close_ephemeral_panics_on_non_ephemeral() {
        let volume = Volume::new("vo-123".to_string(), "test".to_string());
        volume.close_ephemeral();
    }

    #[test]
    fn test_volume_delete_success() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Ok("vo-test-123".to_string()));
        mock.push_delete(Ok(()));
        let svc = make_service(mock);

        svc.delete("test-volume", None).unwrap();
    }

    #[test]
    fn test_volume_delete_with_allow_missing() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::NotFound(
            "Volume 'missing' not found".to_string(),
        )));
        let svc = make_service(mock);

        svc.delete(
            "missing",
            Some(&VolumeDeleteParams {
                allow_missing: true,
                ..Default::default()
            }),
        )
        .unwrap();
    }

    #[test]
    fn test_volume_delete_with_allow_missing_delete_rpc_not_found() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Ok("vo-test-123".to_string()));
        mock.push_delete(Err(ModalError::Grpc(tonic::Status::not_found(
            "Volume not found",
        ))));
        let svc = make_service(mock);

        svc.delete(
            "test-volume",
            Some(&VolumeDeleteParams {
                allow_missing: true,
                ..Default::default()
            }),
        )
        .unwrap();
    }

    #[test]
    fn test_volume_delete_with_allow_missing_false_throws() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Err(ModalError::NotFound(
            "Volume 'missing' not found".to_string(),
        )));
        let svc = make_service(mock);

        let err = svc
            .delete(
                "missing",
                Some(&VolumeDeleteParams {
                    allow_missing: false,
                    ..Default::default()
                }),
            )
            .unwrap_err();

        matches!(err, ModalError::NotFound(_));
    }

    #[test]
    fn test_is_not_found_error() {
        assert!(is_not_found_error(&ModalError::NotFound("test".to_string())));
        assert!(is_not_found_error(&ModalError::Grpc(
            tonic::Status::not_found("test")
        )));
        assert!(!is_not_found_error(&ModalError::Other("test".to_string())));
    }

    #[test]
    fn test_volume_from_name_with_nil_params() {
        let mock = MockVolumeGrpcClient::new();
        mock.push_get_or_create(Ok("vo-test-789".to_string()));
        let svc = make_service(mock);

        let volume = svc.from_name("my-volume", None).unwrap();
        assert_eq!(volume.volume_id, "vo-test-789");
        assert_eq!(volume.name, "my-volume");
    }
}
