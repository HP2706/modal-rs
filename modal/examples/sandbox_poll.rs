// Demonstrates polling a Sandbox's status vs blocking wait.
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

    // Create a sandbox that sleeps briefly then exits with code 42
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "sleep 2; exit 42".to_string(),
                ],
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

    // Poll (non-blocking) — should return None while running
    let poll = client
        .sandboxes
        .poll(&sandbox.sandbox_id)
        .expect("Failed to poll");
    println!("Poll while running: exit_code={:?}", poll.exit_code);

    // Wait (blocking) — returns when done
    let result = client
        .sandboxes
        .wait(&sandbox.sandbox_id, 60.0)
        .expect("Failed to wait");
    println!(
        "Wait result: exit_code={}, success={}",
        result.exit_code, result.success
    );

    // Poll after completion — should return the exit code
    let poll = client
        .sandboxes
        .poll(&sandbox.sandbox_id)
        .expect("Failed to poll");
    println!("Poll after exit: exit_code={:?}", poll.exit_code);

    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
