#![cfg(feature = "integration")]

mod common;

/// Integration tests for gRPC client behavior.
/// Translated from libmodal/modal-go/test/grpc_test.go

use modal::error::ModalError;

#[test]
fn test_grpc_error_types() {
    // Test that gRPC status codes map to correct ModalError variants
    let not_found = ModalError::Grpc(tonic::Status::not_found("resource not found"));
    assert!(matches!(not_found, ModalError::Grpc(ref s) if s.code() == tonic::Code::NotFound));
    assert!(not_found.to_string().contains("resource not found"));

    let internal = ModalError::Grpc(tonic::Status::internal("internal error"));
    assert!(matches!(internal, ModalError::Grpc(ref s) if s.code() == tonic::Code::Internal));

    let deadline = ModalError::Grpc(tonic::Status::deadline_exceeded("timeout"));
    assert!(
        matches!(deadline, ModalError::Grpc(ref s) if s.code() == tonic::Code::DeadlineExceeded)
    );
}

#[test]
fn test_grpc_status_code_matching() {
    let status = tonic::Status::not_found("test");
    assert_eq!(status.code(), tonic::Code::NotFound);
    assert_eq!(status.message(), "test");

    let status = tonic::Status::permission_denied("denied");
    assert_eq!(status.code(), tonic::Code::PermissionDenied);
}

#[test]
fn test_modal_error_display() {
    let err = ModalError::NotFound("Volume 'test' not found".to_string());
    assert!(err.to_string().contains("NotFoundError"));
    assert!(err.to_string().contains("Volume 'test' not found"));

    let err = ModalError::Invalid("bad input".to_string());
    assert!(err.to_string().contains("InvalidError"));

    let err = ModalError::FunctionTimeout("timed out".to_string());
    assert!(err.to_string().contains("FunctionTimeoutError"));
}

#[test]
fn test_modal_error_variants() {
    // Verify all error variants can be constructed and pattern matched
    let errors: Vec<ModalError> = vec![
        ModalError::FunctionTimeout("timeout".to_string()),
        ModalError::Remote("remote error".to_string()),
        ModalError::InternalFailure("internal".to_string()),
        ModalError::Execution("exec error".to_string()),
        ModalError::NotFound("not found".to_string()),
        ModalError::AlreadyExists("exists".to_string()),
        ModalError::Invalid("invalid".to_string()),
        ModalError::QueueEmpty("empty".to_string()),
        ModalError::QueueFull("full".to_string()),
        ModalError::SandboxFilesystem("fs error".to_string()),
        ModalError::SandboxTimeout("sb timeout".to_string()),
        ModalError::ClientClosed("closed".to_string()),
        ModalError::ExecTimeout("exec timeout".to_string()),
        ModalError::Config("config error".to_string()),
        ModalError::Serialization("ser error".to_string()),
        ModalError::Other("other".to_string()),
    ];

    for err in &errors {
        // All errors should have non-empty display
        assert!(!err.to_string().is_empty());
    }
    assert_eq!(errors.len(), 16);
}
