// Demonstrates using ephemeral Secrets in a Sandbox.
// Runs against real Modal API.

use std::collections::HashMap;

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

    // Create an ephemeral secret from a key-value map
    let secret = client
        .secrets
        .from_map(
            &HashMap::from([
                ("MY_KEY".to_string(), "hello-from-rust".to_string()),
                ("MY_NUMBER".to_string(), "42".to_string()),
            ]),
            None,
        )
        .expect("Failed to create ephemeral secret");
    println!("Ephemeral secret: {}", secret.secret_id);

    // Create sandbox with the secret injected as env vars
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo MY_KEY=$MY_KEY MY_NUMBER=$MY_NUMBER".to_string(),
                ],
                secrets: vec![secret],
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

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
