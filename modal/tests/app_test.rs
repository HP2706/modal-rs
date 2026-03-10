#![cfg(feature = "integration")]

/// Integration tests for Modal App module.
/// Translated from libmodal/modal-go/app_test.go

use modal::app::parse_gpu_config;

#[test]
fn test_parse_gpu_config_empty() {
    let config = parse_gpu_config("").unwrap();
    assert_eq!(config.count, 0);
    assert_eq!(config.gpu_type, "");
}

#[test]
fn test_parse_gpu_config_single_types() {
    let cases = vec![
        ("T4", "T4", 1),
        ("A10G", "A10G", 1),
        ("A100-80GB", "A100-80GB", 1),
    ];

    for (input, expected_type, expected_count) in cases {
        let config = parse_gpu_config(input).unwrap();
        assert_eq!(config.gpu_type, expected_type, "input: {}", input);
        assert_eq!(config.count, expected_count, "input: {}", input);
    }
}

#[test]
fn test_parse_gpu_config_with_count() {
    let config = parse_gpu_config("A100-80GB:3").unwrap();
    assert_eq!(config.gpu_type, "A100-80GB");
    assert_eq!(config.count, 3);

    let config = parse_gpu_config("T4:2").unwrap();
    assert_eq!(config.gpu_type, "T4");
    assert_eq!(config.count, 2);
}

#[test]
fn test_parse_gpu_config_case_insensitive() {
    let config = parse_gpu_config("a100:4").unwrap();
    assert_eq!(config.gpu_type, "A100");
    assert_eq!(config.count, 4);
}

#[test]
fn test_parse_gpu_config_invalid_count() {
    let err = parse_gpu_config("T4:invalid").unwrap_err();
    assert!(
        err.to_string().contains("invalid GPU count"),
        "got: {}",
        err
    );
}

#[test]
fn test_parse_gpu_config_empty_count() {
    let err = parse_gpu_config("T4:").unwrap_err();
    assert!(
        err.to_string().contains("invalid GPU count"),
        "got: {}",
        err
    );
}

#[test]
fn test_parse_gpu_config_zero_count() {
    let err = parse_gpu_config("T4:0").unwrap_err();
    assert!(
        err.to_string().contains("invalid GPU count"),
        "got: {}",
        err
    );
}

#[test]
fn test_parse_gpu_config_negative_count() {
    let err = parse_gpu_config("T4:-1").unwrap_err();
    assert!(
        err.to_string().contains("invalid GPU count"),
        "got: {}",
        err
    );
}
