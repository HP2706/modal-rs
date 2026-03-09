#![cfg(feature = "integration")]

/// Integration tests for Modal CloudBucketMount module.
/// Translated from libmodal/modal-go/cloud_bucket_mount_test.go

use modal::cloud_bucket_mount::{
    new_cloud_bucket_mount, BucketType, CloudBucketMountParams,
};
use modal::secret::Secret;

#[test]
fn test_new_cloud_bucket_mount_minimal_options() {
    let mount = new_cloud_bucket_mount("my-bucket", None).unwrap();
    assert_eq!(mount.bucket_name, "my-bucket");
    assert!(!mount.read_only);
    assert!(!mount.requester_pays);
    assert!(mount.secret.is_none());
    assert!(mount.bucket_endpoint_url.is_none());
    assert!(mount.key_prefix.is_none());
    assert!(mount.oidc_auth_role_arn.is_none());
    assert_eq!(mount.bucket_type, BucketType::S3);
}

#[test]
fn test_new_cloud_bucket_mount_all_options() {
    let mock_secret = Secret {
        secret_id: "sec-123".to_string(),
        name: String::new(),
    };
    let params = CloudBucketMountParams {
        secret: Some(mock_secret),
        read_only: true,
        requester_pays: true,
        bucket_endpoint_url: Some("https://my-bucket.r2.cloudflarestorage.com".to_string()),
        key_prefix: Some("prefix/".to_string()),
        oidc_auth_role_arn: Some("arn:aws:iam::123456789:role/MyRole".to_string()),
    };

    let mount = new_cloud_bucket_mount("my-bucket", Some(&params)).unwrap();
    assert_eq!(mount.bucket_name, "my-bucket");
    assert!(mount.read_only);
    assert!(mount.requester_pays);
    assert_eq!(mount.secret.as_ref().unwrap().secret_id, "sec-123");
    assert_eq!(
        mount.bucket_endpoint_url.as_ref().unwrap(),
        "https://my-bucket.r2.cloudflarestorage.com"
    );
    assert_eq!(mount.key_prefix.as_ref().unwrap(), "prefix/");
    assert_eq!(
        mount.oidc_auth_role_arn.as_ref().unwrap(),
        "arn:aws:iam::123456789:role/MyRole"
    );
    assert_eq!(mount.bucket_type, BucketType::R2);
}

#[test]
fn test_bucket_type_detection() {
    struct TestCase {
        name: &'static str,
        endpoint_url: Option<&'static str>,
        expected: BucketType,
    }

    let cases = vec![
        TestCase {
            name: "Empty defaults to S3",
            endpoint_url: None,
            expected: BucketType::S3,
        },
        TestCase {
            name: "R2",
            endpoint_url: Some("https://my-bucket.r2.cloudflarestorage.com"),
            expected: BucketType::R2,
        },
        TestCase {
            name: "GCP",
            endpoint_url: Some("https://storage.googleapis.com/my-bucket"),
            expected: BucketType::Gcp,
        },
        TestCase {
            name: "Unknown defaults to S3",
            endpoint_url: Some("https://unknown-endpoint.com/my-bucket"),
            expected: BucketType::S3,
        },
    ];

    for tc in cases {
        let params = tc.endpoint_url.map(|url| CloudBucketMountParams {
            bucket_endpoint_url: Some(url.to_string()),
            ..Default::default()
        });
        let mount = new_cloud_bucket_mount("my-bucket", params.as_ref()).unwrap();
        assert_eq!(mount.bucket_type, tc.expected, "case: {}", tc.name);
    }
}

#[test]
fn test_new_cloud_bucket_mount_invalid_url() {
    let params = CloudBucketMountParams {
        bucket_endpoint_url: Some("://invalid-url".to_string()),
        ..Default::default()
    };
    let err = new_cloud_bucket_mount("my-bucket", Some(&params)).unwrap_err();
    assert!(
        err.to_string().contains("invalid bucket endpoint URL"),
        "got: {}",
        err
    );
}

#[test]
fn test_new_cloud_bucket_mount_requester_pays_without_secret() {
    let params = CloudBucketMountParams {
        requester_pays: true,
        ..Default::default()
    };
    let err = new_cloud_bucket_mount("my-bucket", Some(&params)).unwrap_err();
    assert!(
        err.to_string()
            .contains("credentials required in order to use Requester Pays"),
        "got: {}",
        err
    );
}

#[test]
fn test_new_cloud_bucket_mount_key_prefix_without_trailing_slash() {
    let params = CloudBucketMountParams {
        key_prefix: Some("prefix".to_string()),
        ..Default::default()
    };
    let err = new_cloud_bucket_mount("my-bucket", Some(&params)).unwrap_err();
    assert!(
        err.to_string().contains("must end in a '/'"),
        "got: {}",
        err
    );
}

#[test]
fn test_cloud_bucket_mount_to_proto_minimal_options() {
    let mount = new_cloud_bucket_mount("my-bucket", None).unwrap();
    let proto = mount.to_proto("/mnt/bucket");

    assert_eq!(proto.bucket_name, "my-bucket");
    assert_eq!(proto.mount_path, "/mnt/bucket");
    assert_eq!(proto.credentials_secret_id, "");
    assert!(!proto.read_only);
    assert_eq!(proto.bucket_type, BucketType::S3);
    assert!(!proto.requester_pays);
    assert_eq!(proto.bucket_endpoint_url, "");
    assert_eq!(proto.key_prefix, "");
    assert_eq!(proto.oidc_auth_role_arn, "");
}

#[test]
fn test_cloud_bucket_mount_to_proto_all_options() {
    let mock_secret = Secret {
        secret_id: "sec-123".to_string(),
        name: String::new(),
    };
    let endpoint_url = "https://my-bucket.r2.cloudflarestorage.com";
    let params = CloudBucketMountParams {
        secret: Some(mock_secret),
        read_only: true,
        requester_pays: true,
        bucket_endpoint_url: Some(endpoint_url.to_string()),
        key_prefix: Some("prefix/".to_string()),
        oidc_auth_role_arn: Some("arn:aws:iam::123456789:role/MyRole".to_string()),
    };

    let mount = new_cloud_bucket_mount("my-bucket", Some(&params)).unwrap();
    let proto = mount.to_proto("/mnt/bucket");

    assert_eq!(proto.bucket_name, "my-bucket");
    assert_eq!(proto.mount_path, "/mnt/bucket");
    assert_eq!(proto.credentials_secret_id, "sec-123");
    assert!(proto.read_only);
    assert_eq!(proto.bucket_type, BucketType::R2);
    assert!(proto.requester_pays);
    assert_eq!(proto.bucket_endpoint_url, endpoint_url);
    assert_eq!(proto.key_prefix, "prefix/");
    assert_eq!(proto.oidc_auth_role_arn, "arn:aws:iam::123456789:role/MyRole");
}
