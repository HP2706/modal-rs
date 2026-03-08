use crate::error::ModalError;
use crate::secret::Secret;

/// Bucket type for cloud storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketType {
    S3,
    R2,
    Gcp,
}

/// CloudBucketMount provides access to cloud storage buckets within Modal Functions.
#[derive(Debug, Clone)]
pub struct CloudBucketMount {
    pub bucket_name: String,
    pub secret: Option<Secret>,
    pub read_only: bool,
    pub requester_pays: bool,
    pub bucket_endpoint_url: Option<String>,
    pub key_prefix: Option<String>,
    pub oidc_auth_role_arn: Option<String>,
    pub bucket_type: BucketType,
}

/// CloudBucketMountParams are options for creating a CloudBucketMount.
#[derive(Debug, Clone, Default)]
pub struct CloudBucketMountParams {
    pub secret: Option<Secret>,
    pub read_only: bool,
    pub requester_pays: bool,
    pub bucket_endpoint_url: Option<String>,
    pub key_prefix: Option<String>,
    pub oidc_auth_role_arn: Option<String>,
}

/// Create a new CloudBucketMount.
pub fn new_cloud_bucket_mount(
    bucket_name: &str,
    params: Option<&CloudBucketMountParams>,
) -> Result<CloudBucketMount, ModalError> {
    let params = params.cloned().unwrap_or_default();

    let bucket_type = if let Some(ref endpoint_url) = params.bucket_endpoint_url {
        let parsed = url::Url::parse(endpoint_url).map_err(|e| {
            ModalError::Invalid(format!("invalid bucket endpoint URL: {}", e))
        })?;
        let hostname = parsed.host_str().unwrap_or("");
        if hostname.ends_with("r2.cloudflarestorage.com") {
            BucketType::R2
        } else if hostname.ends_with("storage.googleapis.com") {
            BucketType::Gcp
        } else {
            BucketType::S3
        }
    } else {
        BucketType::S3
    };

    if params.requester_pays && params.secret.is_none() {
        return Err(ModalError::Invalid(
            "credentials required in order to use Requester Pays".to_string(),
        ));
    }

    if let Some(ref prefix) = params.key_prefix {
        if !prefix.ends_with('/') {
            return Err(ModalError::Invalid(
                "keyPrefix will be prefixed to all object paths, so it must end in a '/'"
                    .to_string(),
            ));
        }
    }

    Ok(CloudBucketMount {
        bucket_name: bucket_name.to_string(),
        secret: params.secret,
        read_only: params.read_only,
        requester_pays: params.requester_pays,
        bucket_endpoint_url: params.bucket_endpoint_url,
        key_prefix: params.key_prefix,
        oidc_auth_role_arn: params.oidc_auth_role_arn,
        bucket_type,
    })
}

/// CloudBucketMountProto is the proto representation.
#[derive(Debug, Clone)]
pub struct CloudBucketMountProto {
    pub bucket_name: String,
    pub mount_path: String,
    pub credentials_secret_id: String,
    pub read_only: bool,
    pub bucket_type: BucketType,
    pub requester_pays: bool,
    pub bucket_endpoint_url: String,
    pub key_prefix: String,
    pub oidc_auth_role_arn: String,
}

impl CloudBucketMount {
    pub fn to_proto(&self, mount_path: &str) -> CloudBucketMountProto {
        CloudBucketMountProto {
            bucket_name: self.bucket_name.clone(),
            mount_path: mount_path.to_string(),
            credentials_secret_id: self
                .secret
                .as_ref()
                .map(|s| s.secret_id.clone())
                .unwrap_or_default(),
            read_only: self.read_only,
            bucket_type: self.bucket_type,
            requester_pays: self.requester_pays,
            bucket_endpoint_url: self.bucket_endpoint_url.clone().unwrap_or_default(),
            key_prefix: self.key_prefix.clone().unwrap_or_default(),
            oidc_auth_role_arn: self.oidc_auth_role_arn.clone().unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cloud_bucket_mount_minimal() {
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
            secret: Some(mock_secret.clone()),
            read_only: true,
            requester_pays: true,
            bucket_endpoint_url: Some(
                "https://my-bucket.r2.cloudflarestorage.com".to_string(),
            ),
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
            endpoint_url: &'static str,
            expected: BucketType,
        }

        let cases = vec![
            TestCase {
                name: "Empty defaults to S3",
                endpoint_url: "",
                expected: BucketType::S3,
            },
            TestCase {
                name: "R2",
                endpoint_url: "https://my-bucket.r2.cloudflarestorage.com",
                expected: BucketType::R2,
            },
            TestCase {
                name: "GCP",
                endpoint_url: "https://storage.googleapis.com/my-bucket",
                expected: BucketType::Gcp,
            },
            TestCase {
                name: "Unknown defaults to S3",
                endpoint_url: "https://unknown-endpoint.com/my-bucket",
                expected: BucketType::S3,
            },
        ];

        for tc in cases {
            let params = if tc.endpoint_url.is_empty() {
                None
            } else {
                Some(CloudBucketMountParams {
                    bucket_endpoint_url: Some(tc.endpoint_url.to_string()),
                    ..Default::default()
                })
            };
            let mount = new_cloud_bucket_mount("my-bucket", params.as_ref()).unwrap();
            assert_eq!(mount.bucket_type, tc.expected, "case: {}", tc.name);
        }
    }

    #[test]
    fn test_invalid_url() {
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
    fn test_requester_pays_without_secret() {
        let params = CloudBucketMountParams {
            requester_pays: true,
            ..Default::default()
        };
        let err = new_cloud_bucket_mount("my-bucket", Some(&params)).unwrap_err();
        assert_eq!(
            err.to_string(),
            "InvalidError: credentials required in order to use Requester Pays"
        );
    }

    #[test]
    fn test_key_prefix_without_trailing_slash() {
        let params = CloudBucketMountParams {
            key_prefix: Some("prefix".to_string()),
            ..Default::default()
        };
        let err = new_cloud_bucket_mount("my-bucket", Some(&params)).unwrap_err();
        assert!(err.to_string().contains("must end in a '/'"));
    }

    #[test]
    fn test_to_proto_minimal() {
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
    fn test_to_proto_all_options() {
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
}
