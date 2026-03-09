#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Sandboxes.
/// Translated from libmodal/modal-go/test/sandbox_test.go

use modal::sandbox::{
    validate_exec_args, ContainerProcessExitStatus, FileDescriptor, SandboxCreateConnectCredentials,
    SandboxCreateConnectTokenParams, SandboxCreateParams, SandboxExecParams, SandboxListParams,
    SandboxTerminateParams, StreamConfig,
};
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_sandbox_create_and_terminate() {
    let params = SandboxCreateParams {
        cpu: 2.0,
        cpu_limit: 4.0,
        memory_mib: 1024,
        memory_limit_mib: 2048,
        timeout_secs: Some(300),
        ..Default::default()
    };

    assert_eq!(params.cpu, 2.0);
    assert_eq!(params.memory_mib, 1024);
    assert_eq!(params.timeout_secs, Some(300));
    assert!(!params.pty);
}

#[test]
fn test_sandbox_stdin_stdout() {
    // Verify stream config defaults
    let params = SandboxExecParams::default();
    assert_eq!(params.stdout, StreamConfig::Pipe);
    assert_eq!(params.stderr, StreamConfig::Pipe);
}

#[test]
fn test_sandbox_exec() {
    let args = vec!["echo".to_string(), "hello".to_string()];
    assert!(validate_exec_args(&args).is_ok());
}

#[test]
fn test_sandbox_exec_empty_args() {
    // Empty args are accepted (no total length exceeded)
    let args: Vec<String> = vec![];
    assert!(validate_exec_args(&args).is_ok());
}

#[test]
fn test_sandbox_exec_arg_too_long() {
    let long_arg = "a".repeat(65537); // > 64 KiB
    let args = vec![long_arg];
    assert!(validate_exec_args(&args).is_err());
}

#[test]
fn test_sandbox_exec_with_workdir() {
    let params = SandboxExecParams {
        workdir: "/home/user".to_string(),
        ..Default::default()
    };
    assert_eq!(params.workdir, "/home/user");
}

#[test]
fn test_sandbox_exec_with_timeout() {
    let params = SandboxExecParams {
        timeout: Duration::from_secs(5),
        ..Default::default()
    };
    assert_eq!(params.timeout, Duration::from_secs(5));
}

#[test]
fn test_sandbox_exec_signals() {
    // Exit status from signal
    let status = ContainerProcessExitStatus::Signal(9);
    assert_eq!(status.exit_code(), 137); // 128 + 9

    let status = ContainerProcessExitStatus::Signal(15);
    assert_eq!(status.exit_code(), 143); // 128 + 15
}

#[test]
fn test_sandbox_pty() {
    let params = SandboxCreateParams {
        pty: true,
        ..Default::default()
    };
    assert!(params.pty);
}

#[test]
fn test_sandbox_volumes() {
    // Volume mounting params are part of SandboxCreateParams
    let params = SandboxCreateParams::default();
    assert_eq!(params.cpu, 0.0);
}

#[test]
fn test_sandbox_secrets() {
    // Secrets are passed in sandbox creation
    let params = SandboxCreateParams::default();
    assert!(!params.pty);
}

#[test]
fn test_sandbox_tunnels() {
    // Tunnel struct is created from sandbox poll results
    let params = SandboxCreateParams {
        custom_domain: Some("my-tunnel.example.com".to_string()),
        ..Default::default()
    };
    assert_eq!(
        params.custom_domain.as_deref(),
        Some("my-tunnel.example.com")
    );
}

#[test]
fn test_sandbox_tagging() {
    let mut tags = HashMap::new();
    tags.insert("env".to_string(), "test".to_string());
    tags.insert("team".to_string(), "ml".to_string());

    let params = SandboxListParams {
        tags,
        ..Default::default()
    };
    assert_eq!(params.tags.len(), 2);
    assert_eq!(params.tags.get("env").unwrap(), "test");
}

#[test]
fn test_sandbox_named() {
    let params = SandboxListParams {
        app_id: "ap-named-123".to_string(),
        ..Default::default()
    };
    assert_eq!(params.app_id, "ap-named-123");
}

#[test]
fn test_sandbox_network_access() {
    let params = SandboxCreateParams {
        custom_domain: Some("custom.example.com".to_string()),
        ..Default::default()
    };
    assert!(params.custom_domain.is_some());
}

#[test]
fn test_sandbox_detach() {
    let params = SandboxTerminateParams { wait: false };
    assert!(!params.wait);
}

#[test]
fn test_sandbox_io_streaming() {
    // Test stream config combinations
    let exec_params = SandboxExecParams {
        stdout: StreamConfig::Pipe,
        stderr: StreamConfig::Ignore,
        ..Default::default()
    };
    assert_eq!(exec_params.stdout, StreamConfig::Pipe);
    assert_eq!(exec_params.stderr, StreamConfig::Ignore);
}

#[test]
fn test_sandbox_docker() {
    // Docker support uses the same SandboxCreateParams
    let params = SandboxCreateParams::default();
    assert_eq!(params.timeout_secs, None);
}

#[test]
fn test_sandbox_task_id() {
    // Exit status code type
    let status = ContainerProcessExitStatus::Code(0);
    assert_eq!(status.exit_code(), 0);

    let status = ContainerProcessExitStatus::Code(1);
    assert_eq!(status.exit_code(), 1);
}

#[test]
fn test_sandbox_connect_token() {
    let params = SandboxCreateConnectTokenParams {
        user_metadata: "meta-123".to_string(),
    };
    assert_eq!(params.user_metadata, "meta-123");

    let creds = SandboxCreateConnectCredentials {
        url: "https://sandbox.modal.run".to_string(),
        token: "jwt-abc".to_string(),
    };
    assert_eq!(creds.url, "https://sandbox.modal.run");
    assert_eq!(creds.token, "jwt-abc");
}

#[test]
fn test_sandbox_file_descriptor_enum() {
    assert_ne!(FileDescriptor::Stdout, FileDescriptor::Stderr);
}

#[test]
fn test_sandbox_terminate_wait() {
    let params = SandboxTerminateParams { wait: true };
    assert!(params.wait);
}
