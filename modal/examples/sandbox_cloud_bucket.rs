// Rust equivalent of examples/sandbox-cloud-bucket (Go).
//
// Demonstrates mounting an AWS S3 bucket to a Sandbox.
// Requires a running Modal backend to execute.

use modal::cloud_bucket_mount::{new_cloud_bucket_mount, CloudBucketMountParams};
use modal::secret::Secret;

fn main() {
    // S3 bucket access requires a secret with AWS credentials.
    let secret = Secret {
        secret_id: "st-aws-secret".to_string(),
        name: "libmodal-aws-bucket-secret".to_string(),
    };

    // Create a cloud bucket mount with key prefix and read-only access.
    let mount = new_cloud_bucket_mount(
        "my-s3-bucket",
        Some(&CloudBucketMountParams {
            secret: Some(secret),
            key_prefix: Some("data/".to_string()),
            read_only: true,
            ..Default::default()
        }),
    )
    .unwrap();

    println!("Bucket: {}", mount.bucket_name);
    println!("Bucket type: {:?}", mount.bucket_type);
    println!("Read only: {}", mount.read_only);
    println!("Key prefix: {:?}", mount.key_prefix);
    println!("Secret: {:?}", mount.secret.as_ref().map(|s| &s.name));

    // With a real client:
    //   let sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       command: vec!["sh", "-c", "ls -la /mnt/s3-bucket"],
    //       cloud_bucket_mounts: HashMap::from([("/mnt/s3-bucket", mount)]),
    //   })?;
    println!("Cloud bucket mount configuration ready.");
}
