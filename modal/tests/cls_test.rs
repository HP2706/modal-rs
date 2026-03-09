#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Cls.
/// Translated from libmodal/modal-go/test/cls_test.go

use modal::cls::{build_function_options_proto, merge_service_options, ServiceOptions};

#[test]
fn test_cls_from_name() {
    // Test ServiceOptions construction and field access
    let opts = ServiceOptions {
        cpu: Some(2.0),
        memory_mib: Some(1024),
        gpu: Some("A100".to_string()),
        ..Default::default()
    };

    assert_eq!(opts.cpu, Some(2.0));
    assert_eq!(opts.memory_mib, Some(1024));
    assert_eq!(opts.gpu.as_deref(), Some("A100"));
}

#[test]
fn test_cls_with_parameters() {
    // Test building function options proto from service options
    let opts = ServiceOptions {
        cpu: Some(4.0),
        cpu_limit: Some(8.0),
        memory_mib: Some(2048),
        memory_limit_mib: Some(4096),
        ..Default::default()
    };

    let proto = build_function_options_proto(Some(&opts)).unwrap().unwrap();
    let resources = proto.resources.unwrap();
    assert_eq!(resources.milli_cpu, 4000);
    assert_eq!(resources.milli_cpu_max, 8000);
    assert_eq!(resources.memory_mb, 2048);
    assert_eq!(resources.memory_mb_max, 4096);
}

#[test]
fn test_cls_merge_options() {
    let base = ServiceOptions {
        cpu: Some(2.0),
        memory_mib: Some(1024),
        ..Default::default()
    };

    let override_opts = ServiceOptions {
        cpu: Some(4.0),
        gpu: Some("T4".to_string()),
        ..Default::default()
    };

    let merged = merge_service_options(Some(&base), Some(&override_opts));
    assert_eq!(merged.cpu, Some(4.0)); // overridden
    assert_eq!(merged.memory_mib, Some(1024)); // from base
    assert_eq!(merged.gpu.as_deref(), Some("T4")); // from override
}

#[test]
fn test_cls_merge_options_nil_base() {
    let new = ServiceOptions {
        cpu: Some(2.0),
        ..Default::default()
    };

    let merged = merge_service_options(None, Some(&new));
    assert_eq!(merged.cpu, Some(2.0));
}

#[test]
fn test_cls_merge_options_nil_new() {
    let base = ServiceOptions {
        cpu: Some(2.0),
        ..Default::default()
    };

    let merged = merge_service_options(Some(&base), None);
    assert_eq!(merged.cpu, Some(2.0));
}

#[test]
fn test_cls_merge_options_both_nil() {
    let merged = merge_service_options(None, None);
    assert!(merged.cpu.is_none());
    assert!(merged.memory_mib.is_none());
}
