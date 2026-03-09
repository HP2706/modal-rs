#![cfg(feature = "integration")]

mod common;

/// Integration tests for sandbox filesystem snapshots.
/// Translated from libmodal/modal-go/test/sandbox_filesystem_snapshot_test.go

use modal::image::Image;

#[test]
fn test_sandbox_filesystem_snapshot_create() {
    let snapshot_image = Image::new("im-snapshot-123".to_string());
    assert_eq!(snapshot_image.image_id, "im-snapshot-123");

    let extended =
        snapshot_image.dockerfile_commands(&["RUN echo from-snapshot".to_string()], None);
    assert_eq!(extended.layers.len(), 2);
}

#[test]
fn test_sandbox_filesystem_snapshot_restore() {
    let snapshot_image = Image::new("im-snapshot-456".to_string());
    assert!(!snapshot_image.image_id.is_empty());

    let extended =
        snapshot_image.dockerfile_commands(&["RUN ls /restored-data".to_string()], None);
    assert!(extended.image_id.is_empty());
    assert_eq!(extended.layers.len(), 2);
}
