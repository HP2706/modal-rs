// Demonstrates using a private AWS ECR image with credentials from a Secret.
// Runs against real Modal API.
//
// Requires:
// - A Modal Secret named "aws-ecr-secret" with AWS credentials
// - A private ECR image accessible with those credentials
//
// Create the secret:
//   modal secret create aws-ecr-secret \
//     AWS_ACCESS_KEY_ID=... AWS_SECRET_ACCESS_KEY=...

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::SandboxCreateParams;
use modal::secret::SecretFromNameParams;

fn main() {
    let ecr_uri = std::env::var("ECR_IMAGE_URI").unwrap_or_else(|_| {
        "459781239556.dkr.ecr.us-east-1.amazonaws.com/ecr-private-registry-test:python".to_string()
    });
    let secret_name =
        std::env::var("ECR_SECRET_NAME").unwrap_or_else(|_| "aws-ecr-secret".to_string());

    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    let app = client
        .apps
        .from_name(
            "libmodal-rs-example",
            Some(&AppFromNameParams {
                create_if_missing: true,
                ..Default::default()
            }),
        )
        .expect("Failed to get or create app");

    // Look up the AWS credentials secret
    let secret = client
        .secrets
        .from_name(&secret_name, Some(&SecretFromNameParams::default()))
        .expect("Failed to find secret — create it with `modal secret create`");

    // Build image from private ECR
    let image = client.images.from_aws_ecr(&ecr_uri, &secret);
    let image = client
        .images
        .build(
            &image,
            &ImageBuildParams {
                app_id: app.app_id.clone(),
                ..Default::default()
            },
        )
        .expect("Failed to build private image");
    println!("Image: {}", image.image_id);

    // Create sandbox from the private image
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'Running from private ECR image'".to_string(),
                ],
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");

    let result = client
        .sandboxes
        .wait(&sandbox.sandbox_id, 60.0)
        .expect("Failed to wait");
    println!(
        "exit_code={}, success={}",
        result.exit_code, result.success
    );

    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
