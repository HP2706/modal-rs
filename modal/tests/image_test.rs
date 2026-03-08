#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Images.
/// Translated from libmodal/modal-go/test/image_test.go

#[test]
fn test_image_from_registry() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create image from Docker registry
}

#[test]
fn test_image_from_aws_ecr() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create image from AWS ECR
}

#[test]
fn test_image_from_gcp_artifact_registry() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create image from GCP Artifact Registry
}

#[test]
fn test_image_dockerfile_commands() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: create image with dockerfile commands
}

#[test]
fn test_image_build() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: build image and verify status
}

#[test]
fn test_image_gpu_config() {
    skip_if_no_credentials!();
    let _client = common::new_test_client().unwrap();
    // TODO: test image with GPU configuration
}
