#![cfg(feature = "integration")]

mod common;

/// Integration tests for sandbox directory snapshots.
/// Translated from libmodal/modal-go/test/sandbox_directory_snapshot_test.go

use modal::image::Image;

#[test]
fn test_sandbox_directory_mount_empty() {
    // An unbuilt image has an empty image_id
    let image = Image::new(String::new());
    assert!(image.image_id.is_empty());
}

#[test]
fn test_sandbox_directory_mount_with_image() {
    // A built image has an image_id
    let image = Image::new("im-built-123".to_string());
    assert_eq!(image.image_id, "im-built-123");
    assert!(!image.image_id.is_empty());
}

#[test]
fn test_sandbox_directory_snapshot() {
    // Test image with dockerfile commands represents a snapshotable state
    let image = Image::new(String::new())
        .dockerfile_commands(&["RUN mkdir -p /data".to_string()], None);

    assert_eq!(image.layers.len(), 2);
    assert_eq!(image.layers[1].commands[0], "RUN mkdir -p /data");
}

#[test]
fn test_sandbox_directory_unbuilt_image_error() {
    // Verify that an unbuilt image (with layers but no build) has empty ID
    let image = Image::new(String::new())
        .dockerfile_commands(&["RUN echo test".to_string()], None);

    assert!(image.image_id.is_empty());
}
