// Rust equivalent of examples/image-building (Go).
//
// Demonstrates building and caching a Docker image with custom system
// dependencies using Image.dockerfile_commands() and layer chaining.
// Runs against real Modal API.

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;

fn main() {
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
    println!("App: {} ({})", app.name, app.app_id);

    // Build a base image from registry
    let base = client.images.from_registry("alpine:3.21", None);
    println!("Base image tag: {}", base.tag);

    // Add a layer with Dockerfile commands
    let image = base.dockerfile_commands(
        &[
            "RUN apk add --no-cache curl".to_string(),
            "RUN apk add --no-cache jq".to_string(),
        ],
        None,
    );
    println!("Image layers: {}", image.layers.len());
    for (i, layer) in image.layers.iter().enumerate() {
        println!("  Layer {}: {:?}", i, layer.commands);
    }

    // Build the image on Modal
    println!("Building image on Modal...");
    let built = client
        .images
        .build(
            &image,
            &ImageBuildParams {
                app_id: app.app_id.clone(),
                ..Default::default()
            },
        )
        .expect("Failed to build image");
    println!("Image built: {}", built.image_id);

    // Use the built image in a sandbox to verify curl and jq are installed
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &built.image_id,
            modal::sandbox::SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "curl --version | head -1 && jq --version".to_string(),
                ],
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

    let result = client
        .sandboxes
        .wait(&sandbox.sandbox_id, 60.0)
        .expect("Failed to wait for sandbox");
    println!(
        "Sandbox finished - exit_code: {}, success: {}",
        result.exit_code, result.success
    );

    client
        .sandboxes
        .terminate(&sandbox.sandbox_id)
        .expect("Failed to terminate sandbox");
    println!("Done!");
}
