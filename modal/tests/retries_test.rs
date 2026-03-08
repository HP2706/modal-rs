#![cfg(feature = "integration")]

mod common;

use modal::retries::{Retries, RetriesParams};
use std::time::Duration;

/// Integration tests for retry configuration validation.
/// Translated from libmodal/modal-go/test/retries_test.go

#[test]
fn test_retries_valid_config() {
    let r = Retries::new(3, None).unwrap();
    assert_eq!(r.max_retries, 3);
    assert_eq!(r.backoff_coefficient, 2.0);
    assert_eq!(r.initial_delay, Duration::from_secs(1));
    assert_eq!(r.max_delay, Duration::from_secs(60));
}

#[test]
fn test_retries_custom_config() {
    let params = RetriesParams {
        backoff_coefficient: Some(3.0),
        initial_delay: Some(Duration::from_millis(500)),
        max_delay: Some(Duration::from_secs(30)),
    };
    let r = Retries::new(5, Some(&params)).unwrap();
    assert_eq!(r.max_retries, 5);
    assert_eq!(r.backoff_coefficient, 3.0);
    assert_eq!(r.initial_delay, Duration::from_millis(500));
    assert_eq!(r.max_delay, Duration::from_secs(30));
}

#[test]
fn test_retries_invalid_max_retries() {
    assert!(Retries::new(-1, None).is_err());
    assert!(Retries::new(11, None).is_err());
}

#[test]
fn test_retries_invalid_backoff() {
    let params = RetriesParams {
        backoff_coefficient: Some(0.5),
        ..Default::default()
    };
    assert!(Retries::new(3, Some(&params)).is_err());

    let params = RetriesParams {
        backoff_coefficient: Some(11.0),
        ..Default::default()
    };
    assert!(Retries::new(3, Some(&params)).is_err());
}

#[test]
fn test_retries_invalid_delays() {
    let params = RetriesParams {
        initial_delay: Some(Duration::from_secs(61)),
        ..Default::default()
    };
    assert!(Retries::new(3, Some(&params)).is_err());

    let params = RetriesParams {
        max_delay: Some(Duration::from_millis(500)),
        ..Default::default()
    };
    assert!(Retries::new(3, Some(&params)).is_err());
}

#[test]
fn test_retries_boundary_values() {
    assert!(Retries::new(0, None).is_ok());
    assert!(Retries::new(10, None).is_ok());

    let params = RetriesParams {
        backoff_coefficient: Some(1.0),
        ..Default::default()
    };
    assert!(Retries::new(3, Some(&params)).is_ok());

    let params = RetriesParams {
        backoff_coefficient: Some(10.0),
        ..Default::default()
    };
    assert!(Retries::new(3, Some(&params)).is_ok());
}
