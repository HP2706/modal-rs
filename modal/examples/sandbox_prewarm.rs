// Demonstrates pre-building an image to speed up sandbox creation.
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

    // Pre-build the image (this caches it on Modal)
    let base = client.images.from_registry("alpine:3.21", None);
    let image = base.dockerfile_commands(
        &["RUN apk add --no-cache curl jq".to_string()],
        None,
    );

    println!("Building image (first time may be slow)...");
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

    // Reference the pre-built image by ID (instant, no rebuild)
    let from_id = client
        .images
        .from_id(&built.image_id)
        .expect("Failed to get image by ID");
    println!("Image from ID: {}", from_id.image_id);

    // Create sandbox using the pre-warmed image
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &from_id.image_id,
            SandboxCreateParams {
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

    let result = client
        .sandboxes
        .wait(&sandbox.sandbox_id, 60.0)
        .expect("Failed to wait");
    println!("exit_code={}, success={}", result.exit_code, result.success);

    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
