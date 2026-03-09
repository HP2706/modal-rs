#![cfg(feature = "integration")]

/// Integration tests for Modal Logger module.
/// Translated from libmodal/modal-go/logger_test.go

use modal::logger::{parse_log_level, LogLevel};

#[test]
fn test_parse_log_level_valid_values() {
    let cases = vec![
        ("DEBUG", LogLevel::Debug),
        ("INFO", LogLevel::Info),
        ("WARN", LogLevel::Warn),
        ("WARNING", LogLevel::Warn),
        ("ERROR", LogLevel::Error),
        ("eRrOr", LogLevel::Error),
        ("", LogLevel::Warn),
    ];

    for (input, expected) in cases {
        let level = parse_log_level(input).unwrap();
        assert_eq!(level, expected, "input: {:?}", input);
    }
}

#[test]
fn test_parse_log_level_invalid_value() {
    let err = parse_log_level("invalid").unwrap_err();
    assert!(
        err.to_string().contains("invalid log level"),
        "got: {}",
        err
    );
}
