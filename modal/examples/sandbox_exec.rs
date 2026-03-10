// Rust equivalent of examples/sandbox-exec (Go).
//
// Creates a real Sandbox on Modal and executes commands in it.
// Requires valid Modal credentials in ~/.modal.toml or environment variables.

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::{SandboxCreateParams, SandboxExecParams};

fn main() {
    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

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

    // Build a Python image
    let image = client.images.from_registry("python:3.13-slim", None);
    println!("Building image...");
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

    // Create a sandbox (no initial command — it stays alive for exec calls)
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Created sandbox: {}", sandbox.sandbox_id);

    // Execute a Python command
    let exec_id = client
        .sandboxes
        .exec(
            &sandbox,
            vec![
                "python".to_string(),
                "-c".to_string(),
                "print('Hello from Python in modal-rs!')".to_string(),
            ],
            SandboxExecParams::default(),
        )
        .expect("Failed to exec in sandbox");
    println!("Exec ID: {}", exec_id);

    // Wait for exec to finish
    let exec_result = client
        .sandboxes
        .exec_wait(&exec_id, 60.0)
        .expect("Failed to wait for exec");
    println!(
        "Exec finished - exit_code: {:?}, completed: {}",
        exec_result.exit_code, exec_result.completed
    );

    // Execute another command — list Python version
    let exec_id2 = client
        .sandboxes
        .exec(
            &sandbox,
            vec!["python".to_string(), "--version".to_string()],
            SandboxExecParams::default(),
        )
        .expect("Failed to exec python --version");
    let exec_result2 = client
        .sandboxes
        .exec_wait(&exec_id2, 60.0)
        .expect("Failed to wait for python --version");
    println!(
        "Python version exec - exit_code: {:?}, completed: {}",
        exec_result2.exit_code, exec_result2.completed
    );

    // Terminate the sandbox
    client
        .sandboxes
        .terminate(&sandbox.sandbox_id)
        .expect("Failed to terminate sandbox");
    println!("Sandbox terminated.");
}
