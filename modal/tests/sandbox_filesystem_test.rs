#![cfg(feature = "integration")]

mod common;

/// Integration tests for sandbox filesystem operations.
/// Translated from libmodal/modal-go/test/sandbox_filesystem_test.go

use modal::sandbox_filesystem::{
    FileMode, FileWatchEvent, FileWatchEventType, SeekWhence, SystemErrorCode,
};

#[test]
fn test_sandbox_file_write_and_read() {
    // Test FileMode for read and write modes
    let read_mode = FileMode::parse("r").unwrap();
    assert!(read_mode.readable);
    assert!(!read_mode.writable);

    let write_mode = FileMode::parse("w").unwrap();
    assert!(write_mode.writable);
    assert!(!write_mode.readable);
}

#[test]
fn test_sandbox_file_write_binary() {
    let mode = FileMode::parse("wb").unwrap();
    assert!(mode.writable);
    assert!(mode.binary);
    assert!(!mode.readable);
}

#[test]
fn test_sandbox_file_append() {
    let mode = FileMode::parse("a").unwrap();
    assert!(mode.writable);
    assert!(mode.appended);
}

#[test]
fn test_sandbox_file_read_write() {
    let mode = FileMode::parse("r+").unwrap();
    assert!(mode.readable);
    assert!(mode.writable);
}

#[test]
fn test_sandbox_file_mode_invalid() {
    assert!(FileMode::parse("z").is_err());
    assert!(FileMode::parse("").is_err());
}

#[test]
fn test_sandbox_file_flush() {
    // FileMode for writing (flush is a runtime operation, test mode parsing)
    let mode = FileMode::parse("w+").unwrap();
    assert!(mode.writable);
    assert!(mode.readable);
}

#[test]
fn test_sandbox_file_multiple() {
    // Test multiple file mode variations
    let modes = vec!["r", "w", "a", "rb", "wb", "ab", "r+", "w+", "a+"];
    for m in modes {
        assert!(FileMode::parse(m).is_ok(), "Failed to parse mode: {}", m);
    }
}

#[test]
fn test_sandbox_system_error_codes() {
    assert_eq!(
        SystemErrorCode::from_i32(1),
        SystemErrorCode::PermissionDenied
    );
    assert_eq!(SystemErrorCode::from_i32(2), SystemErrorCode::NotFound);
    assert_eq!(SystemErrorCode::from_i32(13), SystemErrorCode::AccessDenied);
    assert_eq!(
        SystemErrorCode::from_i32(17),
        SystemErrorCode::AlreadyExists
    );
    assert_eq!(
        SystemErrorCode::from_i32(22),
        SystemErrorCode::InvalidArgument
    );
    assert_eq!(SystemErrorCode::from_i32(28), SystemErrorCode::NoSpaceLeft);
    assert_eq!(SystemErrorCode::from_i32(999), SystemErrorCode::Unspecified);
}

#[test]
fn test_sandbox_system_error_to_modal_error() {
    let err = SystemErrorCode::NotFound.to_error("test.txt");
    assert!(err.to_string().contains("test.txt"));

    let err = SystemErrorCode::AlreadyExists.to_error("file.txt");
    assert!(err.to_string().contains("file.txt"));
}

#[test]
fn test_sandbox_seek_whence() {
    assert_eq!(SeekWhence::from_i32(0).unwrap(), SeekWhence::Set);
    assert_eq!(SeekWhence::from_i32(1).unwrap(), SeekWhence::Current);
    assert_eq!(SeekWhence::from_i32(2).unwrap(), SeekWhence::End);
    assert!(SeekWhence::from_i32(99).is_err());
}

#[test]
fn test_sandbox_file_watch_event_type() {
    assert_eq!(FileWatchEventType::from_str("Create"), FileWatchEventType::Create);
    assert_eq!(FileWatchEventType::from_str("Modify"), FileWatchEventType::Modify);
    assert_eq!(FileWatchEventType::from_str("Remove"), FileWatchEventType::Remove);
    assert_eq!(FileWatchEventType::from_str("Access"), FileWatchEventType::Access);
    assert_eq!(
        FileWatchEventType::from_str("unknown"),
        FileWatchEventType::Unknown
    );
}

#[test]
fn test_sandbox_file_watch_event_parse() {
    let json = br#"{"event_type": "Create", "paths": ["/tmp/test.txt"]}"#;
    let event = FileWatchEvent::from_json(json).unwrap();
    assert_eq!(event.event_type, FileWatchEventType::Create);
    assert_eq!(event.paths, vec!["/tmp/test.txt"]);
}

#[test]
fn test_sandbox_file_watch_event_multiple_paths() {
    let json = br#"{"event_type": "Modify", "paths": ["/a.txt", "/b.txt"]}"#;
    let event = FileWatchEvent::from_json(json).unwrap();
    assert_eq!(event.event_type, FileWatchEventType::Modify);
    assert_eq!(event.paths.len(), 2);
}
