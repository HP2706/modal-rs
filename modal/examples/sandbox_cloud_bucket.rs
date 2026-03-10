// Demonstrates creating a cloud bucket mount configuration.
// Runs against real Modal API.
//
// Requires:
// - A Modal Secret named "aws-bucket-secret" with AWS credentials
//   (AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)
//
// Note: The Rust SDK can create cloud bucket mounts and look up secrets,
// but SandboxCreateParams does not yet support the cloud_bucket_mounts field.
// This example demonstrates the mount creation and secret lookup.

use modal::client::Client;
use modal::cloud_bucket_mount::CloudBucketMountParams;
use modal::secret::SecretFromNameParams;

fn main() {
    let bucket_name =
        std::env::var("S3_BUCKET_NAME").unwrap_or_else(|_| "my-test-bucket".to_string());
    let secret_name =
        std::env::var("S3_SECRET_NAME").unwrap_or_else(|_| "aws-bucket-secret".to_string());

    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    // Look up the AWS credentials secret
    let secret = client
        .secrets
        .from_name(&secret_name, Some(&SecretFromNameParams::default()))
        .expect("Failed to find secret — create it with `modal secret create`");
    println!("Secret: {}", secret.secret_id);

    // Create a cloud bucket mount
    let mount = client
        .cloud_bucket_mounts
        .new_mount(
            &bucket_name,
            Some(&CloudBucketMountParams {
                secret: Some(secret),
                read_only: true,
                ..Default::default()
            }),
        )
        .expect("Failed to create cloud bucket mount");

    println!("Bucket: {}", mount.bucket_name);
    println!("Read only: {}", mount.read_only);

    // TODO: Once SandboxCreateParams supports cloud_bucket_mounts:
    //   let sandbox = client.sandboxes.create(&app_id, &image_id, SandboxCreateParams {
    //       cloud_bucket_mounts: HashMap::from([("/mnt/s3-bucket", mount)]),
    //       command: vec!["ls", "-la", "/mnt/s3-bucket"],
    //       ..Default::default()
    //   })?;

    println!("Done!");
}
