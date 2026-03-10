// Demonstrates creating a Sandbox with GPU access.
// Runs against real Modal API.
//
// Requires: GPU quota on your Modal account (A10G or similar).
// Will fail with a scheduling error if no GPU quota is available.

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::SandboxCreateParams;

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

    // Use NVIDIA CUDA image for GPU workloads
    let base = client
        .images
        .from_registry("nvidia/cuda:12.4.0-devel-ubuntu22.04", None);
    let image = client
        .images
        .build(
            &base,
            &ImageBuildParams {
                app_id: app.app_id.clone(),
                ..Default::default()
            },
        )
        .expect("Failed to build image");

    // Create sandbox with GPU
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec!["nvidia-smi".to_string()],
                gpu: "A10G".to_string(),
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create GPU sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

    let result = client
        .sandboxes
        .wait(&sandbox.sandbox_id, 120.0)
        .expect("Failed to wait");
    println!(
        "exit_code={}, success={}",
        result.exit_code, result.success
    );

    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
