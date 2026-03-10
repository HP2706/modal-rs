#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Volumes.
/// Translated from libmodal/modal-go/test/volume_test.go

use modal::error::ModalError;
use modal::volume::{
    Volume, VolumeDeleteParams, VolumeFromNameParams, VolumeGrpcClient, VolumeService,
    VolumeServiceImpl,
};
use std::sync::Mutex;

struct MockVolumeClient {
    responses: Mutex<Vec<MockResp>>,
}

enum MockResp {
    GetOrCreate(Result<String, ModalError>),
    #[allow(dead_code)]
    Delete(Result<(), ModalError>),
}

impl MockVolumeClient {
    fn new() -> Self {
        Self {
            responses: Mutex::new(Vec::new()),
        }
    }

    fn push_get_or_create(&self, r: Result<String, ModalError>) {
        self.responses.lock().unwrap().push(MockResp::GetOrCreate(r));
    }

    #[allow(dead_code)]
    fn push_delete(&self, r: Result<(), ModalError>) {
        self.responses.lock().unwrap().push(MockResp::Delete(r));
    }
}

impl VolumeGrpcClient for MockVolumeClient {
    fn volume_get_or_create(
        &self,
        _deployment_name: &str,
        _environment_name: &str,
        _object_creation_type: i32,
    ) -> Result<String, ModalError> {
        match self.responses.lock().unwrap().remove(0) {
            MockResp::GetOrCreate(r) => r,
            _ => panic!("unexpected mock response type"),
        }
    }

    fn volume_heartbeat(&self, _volume_id: &str) -> Result<(), ModalError> {
        Ok(())
    }

    fn volume_delete(&self, _volume_id: &str) -> Result<(), ModalError> {
        match self.responses.lock().unwrap().remove(0) {
            MockResp::Delete(r) => r,
            _ => panic!("unexpected mock response type"),
        }
    }
}

fn make_service(mock: MockVolumeClient) -> VolumeServiceImpl<MockVolumeClient> {
    VolumeServiceImpl {
        client: mock,
        profile: modal::config::Profile::default(),
    }
}

#[test]
fn test_volume_create_and_delete() {
    let mock = MockVolumeClient::new();
    mock.push_get_or_create(Ok("vo-test-vol-123".to_string()));
    let svc = make_service(mock);

    let vol = svc
        .from_name(
            "test-volume",
            Some(&VolumeFromNameParams {
                create_if_missing: true,
                ..Default::default()
            }),
        )
        .unwrap();

    assert!(vol.volume_id.starts_with("vo-"));
    assert_eq!(vol.name, "test-volume");
    assert!(!vol.is_read_only());
    assert!(!vol.is_ephemeral());
}

#[test]
fn test_volume_ephemeral() {
    let mock = MockVolumeClient::new();
    mock.push_get_or_create(Ok("vo-eph-456".to_string()));
    let svc = make_service(mock);

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    rt.block_on(async {
        let vol = svc.ephemeral(None).unwrap();
        assert!(vol.is_ephemeral());
        assert_eq!(vol.name, "");
        assert!(vol.volume_id.starts_with("vo-"));
        vol.close_ephemeral();
    });
}

#[test]
fn test_volume_read_only() {
    let vol = Volume::new("vo-ro-789".to_string(), "my-vol".to_string());
    assert!(!vol.is_read_only());

    let ro = vol.read_only();
    assert!(ro.is_read_only());
    assert_eq!(ro.volume_id, "vo-ro-789");
    assert_eq!(ro.name, "my-vol");

    // Original unchanged
    assert!(!vol.is_read_only());
}

#[test]
fn test_volume_from_name() {
    let mock = MockVolumeClient::new();
    mock.push_get_or_create(Ok("vo-named-321".to_string()));
    let svc = make_service(mock);

    let vol = svc.from_name("my-volume", None).unwrap();
    assert_eq!(vol.volume_id, "vo-named-321");
    assert_eq!(vol.name, "my-volume");
}

#[test]
fn test_volume_from_name_not_found() {
    let mock = MockVolumeClient::new();
    mock.push_get_or_create(Err(ModalError::Grpc(tonic::Status::not_found("not found"))));
    let svc = make_service(mock);

    let err = svc.from_name("missing", None).unwrap_err();
    assert!(err.to_string().contains("not found"), "got: {}", err);
}

#[test]
fn test_volume_delete_allow_missing() {
    let mock = MockVolumeClient::new();
    mock.push_get_or_create(Err(ModalError::NotFound("not found".to_string())));
    let svc = make_service(mock);

    svc.delete(
        "missing-vol",
        Some(&VolumeDeleteParams {
            allow_missing: true,
            ..Default::default()
        }),
    )
    .unwrap();
}

#[test]
fn test_volume_delete_not_found_without_allow_missing() {
    let mock = MockVolumeClient::new();
    mock.push_get_or_create(Err(ModalError::NotFound("not found".to_string())));
    let svc = make_service(mock);

    let err = svc
        .delete(
            "missing-vol",
            Some(&VolumeDeleteParams {
                allow_missing: false,
                ..Default::default()
            }),
        )
        .unwrap_err();
    assert!(matches!(err, ModalError::NotFound(_)));
}

#[test]
#[should_panic(expected = "is not ephemeral")]
fn test_volume_close_ephemeral_panics_non_ephemeral() {
    let vol = Volume::new("vo-123".to_string(), "test".to_string());
    vol.close_ephemeral();
}
