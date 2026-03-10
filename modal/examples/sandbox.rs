// Rust equivalent of examples/sandbox (Go).
//
// Creates a real Sandbox on Modal with stdin/stdout communication.
// Requires valid Modal credentials in ~/.modal.toml or environment variables.

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::SandboxCreateParams;

fn main() {
    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");
    println!("Connected (version: {})", client.version());

    // Get or create the app
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

    // Create an image from a public registry
    let image = client.images.from_registry("alpine:3.21", None);
    println!("Building image (tag: {})...", image.tag);

    let image = client
        .images
        .build(
            &image,
            &ImageBuildParams {
                app_id: app.app_id.clone(),
                ..Default::default()
            },
        )
        .expect("Failed to build image");
    println!("Image built: {}", image.image_id);

    // Create a sandbox running "echo hello from modal-rs"
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "echo".to_string(),
                    "hello from modal-rs!".to_string(),
                ],
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Created sandbox: {}", sandbox.sandbox_id);

    // Wait for the sandbox to finish
    let result = client
        .sandboxes
        .wait(&sandbox.sandbox_id, 60.0)
        .expect("Failed to wait for sandbox");
    println!(
        "Sandbox finished - exit_code: {}, success: {}",
        result.exit_code, result.success
    );

    // Clean up
    client
        .sandboxes
        .terminate(&sandbox.sandbox_id)
        .expect("Failed to terminate sandbox");
    println!("Sandbox terminated.");
}
