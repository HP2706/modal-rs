#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Sandboxes.
/// Translated from libmodal/modal-go/test/sandbox_test.go (1417 lines)

#[test]
fn test_sandbox_create_and_terminate() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox, verify it runs, terminate it
}

#[test]
fn test_sandbox_stdin_stdout() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox, write to stdin, read from stdout
}

#[test]
fn test_sandbox_exec() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox, exec a command, check exit code
}

#[test]
fn test_sandbox_exec_with_workdir() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: exec with workdir option
}

#[test]
fn test_sandbox_exec_with_timeout() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: exec with timeout that expires
}

#[test]
fn test_sandbox_exec_signals() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: send signal to exec process
}

#[test]
fn test_sandbox_pty() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox with PTY enabled
}

#[test]
fn test_sandbox_volumes() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox with mounted volumes
}

#[test]
fn test_sandbox_secrets() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox with secrets
}

#[test]
fn test_sandbox_tunnels() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox with tunnels (encrypted/unencrypted)
}

#[test]
fn test_sandbox_tagging() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create sandbox with tags, list by tag
}

#[test]
fn test_sandbox_named() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create named sandbox
}

#[test]
fn test_sandbox_network_access() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test sandbox with CIDR allowlist
}

#[test]
fn test_sandbox_detach() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: detach from sandbox, reconnect
}

#[test]
fn test_sandbox_io_streaming() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test stdin/stdout streaming
}

#[test]
fn test_sandbox_docker() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test experimental Docker support
}

#[test]
fn test_sandbox_task_id() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test task ID polling
}

#[test]
fn test_sandbox_connect_token() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test connect token
}
