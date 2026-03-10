// Demonstrates creating a named Sandbox and retrieving it by name.
// Runs against real Modal API.

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

    let image = client.images.from_registry("alpine:3.21", None);
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

    let sandbox_name = format!("rs-named-example-{}", std::process::id());

    // Create a named sandbox
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'I am a named sandbox'; sleep 5".to_string(),
                ],
                name: sandbox_name.clone(),
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create named sandbox");
    println!("Created named sandbox: {} (name={})", sandbox.sandbox_id, sandbox_name);

    // Retrieve it by name
    let found = client
        .sandboxes
        .from_name("libmodal-rs-example", &sandbox_name, None)
        .expect("Failed to find sandbox by name");
    println!("Found by name: {}", found.sandbox_id);
    assert_eq!(sandbox.sandbox_id, found.sandbox_id);

    // Clean up
    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
