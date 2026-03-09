// Rust equivalent of examples/sandbox-private-image (Go).
//
// Demonstrates using a private AWS ECR image with credentials from a Secret.
// Requires a running Modal backend to execute.

use modal::image::{Image, ImageRegistryConfig, RegistryAuthType};
use modal::secret::Secret;

fn main() {
    // AWS ECR requires a Secret with AWS credentials.
    let secret = Secret {
        secret_id: "st-ecr-secret".to_string(),
        name: "libmodal-aws-ecr-test".to_string(),
    };

    // Image from AWS ECR with registry authentication.
    let image = Image {
        image_id: String::new(),
        image_registry_config: Some(ImageRegistryConfig {
            registry_auth_type: RegistryAuthType::Aws,
            secret_id: secret.secret_id.clone(),
        }),
        tag: "459781239556.dkr.ecr.us-east-1.amazonaws.com/ecr-private-registry-test:python"
            .to_string(),
        layers: vec![Default::default()],
    };
    println!("Private ECR image tag: {}", image.tag);
    println!(
        "Auth type: {:?}",
        image.image_registry_config.as_ref().unwrap().registry_auth_type
    );

    // GCP Artifact Registry uses RegistryAuthType::Gcp instead.
    let gcp_image = Image {
        image_id: String::new(),
        image_registry_config: Some(ImageRegistryConfig {
            registry_auth_type: RegistryAuthType::Gcp,
            secret_id: "st-gcp-secret".to_string(),
        }),
        tag: "us-docker.pkg.dev/project/repo/image:latest".to_string(),
        layers: vec![Default::default()],
    };
    println!("GCP image auth: {:?}", gcp_image.image_registry_config.as_ref().unwrap().registry_auth_type);

    // With a real client:
    //   let image = image_service.from_aws_ecr(tag, &secret);
    //   let sb = sandbox_service.create(app, image, &params)?;
    println!("Private image configuration ready.");
}
