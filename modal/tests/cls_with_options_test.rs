#![cfg(feature = "integration")]

mod common;

/// Integration tests for Modal Cls with options.
/// Translated from libmodal/modal-go/test/cls_with_options_test.go

use modal::cls::{build_function_options_proto, merge_service_options, ServiceOptions};
use modal::retries::Retries;
use modal::secret::Secret;
use modal::volume::Volume;
use std::collections::HashMap;
use std::time::Duration;

#[test]
fn test_cls_with_timeout_option() {
    let opts = ServiceOptions {
        timeout: Some(Duration::from_secs(30)),
        ..Default::default()
    };
    assert_eq!(opts.timeout, Some(Duration::from_secs(30)));
}

#[test]
fn test_cls_with_cpu_option() {
    let opts = ServiceOptions {
        cpu: Some(2.0),
        ..Default::default()
    };
    let proto = build_function_options_proto(Some(&opts)).unwrap().unwrap();
    assert_eq!(proto.resources.unwrap().milli_cpu, 2000);
}

#[test]
fn test_cls_with_memory_option() {
    let opts = ServiceOptions {
        memory_mib: Some(2048),
        ..Default::default()
    };
    let proto = build_function_options_proto(Some(&opts)).unwrap().unwrap();
    assert_eq!(proto.resources.unwrap().memory_mb, 2048);
}

#[test]
fn test_cls_with_gpu_option() {
    let opts = ServiceOptions {
        gpu: Some("A100".to_string()),
        ..Default::default()
    };
    // GPU option should produce a proto result (resources present)
    let proto = build_function_options_proto(Some(&opts)).unwrap().unwrap();
    assert!(proto.resources.is_some());
}

#[test]
fn test_cls_with_secrets_option() {
    let opts = ServiceOptions {
        secrets: Some(vec![
            Secret {
                secret_id: "st-1".to_string(),
                name: "secret-a".to_string(),
            },
            Secret {
                secret_id: "st-2".to_string(),
                name: "secret-b".to_string(),
            },
        ]),
        ..Default::default()
    };
    assert_eq!(opts.secrets.as_ref().unwrap().len(), 2);
}

#[test]
fn test_cls_with_volumes_option() {
    let mut volumes = HashMap::new();
    volumes.insert(
        "/data".to_string(),
        Volume::new("vo-1".to_string(), "data-vol".to_string()),
    );
    let opts = ServiceOptions {
        volumes: Some(volumes),
        ..Default::default()
    };
    assert_eq!(opts.volumes.as_ref().unwrap().len(), 1);
}

#[test]
fn test_cls_with_concurrency() {
    let opts = ServiceOptions {
        max_concurrent_inputs: Some(10),
        target_concurrent_inputs: Some(5),
        ..Default::default()
    };
    assert_eq!(opts.max_concurrent_inputs, Some(10));
    assert_eq!(opts.target_concurrent_inputs, Some(5));
}

#[test]
fn test_cls_with_batching() {
    let opts = ServiceOptions {
        batch_max_size: Some(32),
        batch_wait: Some(Duration::from_millis(100)),
        ..Default::default()
    };
    assert_eq!(opts.batch_max_size, Some(32));
    assert_eq!(opts.batch_wait, Some(Duration::from_millis(100)));
}

#[test]
fn test_cls_with_retries() {
    let retries = Retries::new(3, None).unwrap();
    let opts = ServiceOptions {
        retries: Some(retries),
        ..Default::default()
    };
    assert_eq!(opts.retries.as_ref().unwrap().max_retries, 3);
}

#[test]
fn test_cls_option_stacking() {
    let base = ServiceOptions {
        cpu: Some(1.0),
        memory_mib: Some(512),
        timeout: Some(Duration::from_secs(10)),
        ..Default::default()
    };

    let layer1 = ServiceOptions {
        cpu: Some(2.0),
        gpu: Some("T4".to_string()),
        ..Default::default()
    };

    let layer2 = ServiceOptions {
        memory_mib: Some(2048),
        max_containers: Some(5),
        ..Default::default()
    };

    let merged = merge_service_options(Some(&base), Some(&layer1));
    let final_opts = merge_service_options(Some(&merged), Some(&layer2));

    assert_eq!(final_opts.cpu, Some(2.0)); // from layer1
    assert_eq!(final_opts.memory_mib, Some(2048)); // from layer2
    assert_eq!(final_opts.gpu.as_deref(), Some("T4")); // from layer1
    assert_eq!(final_opts.timeout, Some(Duration::from_secs(10))); // from base
    assert_eq!(final_opts.max_containers, Some(5)); // from layer2
}

#[test]
fn test_cls_cpu_validation_negative() {
    let opts = ServiceOptions {
        cpu: Some(-1.0),
        ..Default::default()
    };
    assert!(build_function_options_proto(Some(&opts)).is_err());
}

#[test]
fn test_cls_cpu_limit_without_cpu() {
    let opts = ServiceOptions {
        cpu_limit: Some(4.0),
        ..Default::default()
    };
    assert!(build_function_options_proto(Some(&opts)).is_err());
}

#[test]
fn test_cls_cpu_limit_less_than_cpu() {
    let opts = ServiceOptions {
        cpu: Some(4.0),
        cpu_limit: Some(2.0),
        ..Default::default()
    };
    assert!(build_function_options_proto(Some(&opts)).is_err());
}

#[test]
fn test_cls_memory_limit_without_memory() {
    let opts = ServiceOptions {
        memory_limit_mib: Some(2048),
        ..Default::default()
    };
    assert!(build_function_options_proto(Some(&opts)).is_err());
}

#[test]
fn test_cls_memory_limit_less_than_memory() {
    let opts = ServiceOptions {
        memory_mib: Some(2048),
        memory_limit_mib: Some(1024),
        ..Default::default()
    };
    assert!(build_function_options_proto(Some(&opts)).is_err());
}

#[test]
fn test_cls_nil_options() {
    let result = build_function_options_proto(None).unwrap();
    assert!(result.is_none());
}
