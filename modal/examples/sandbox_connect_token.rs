// Demonstrates creating Connect Tokens for secure HTTP access to a Sandbox.
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

    // Start a simple HTTP server on port 8080 (Connect Token default port)
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    // Simple HTTP responder using shell (no extra packages needed)
                    "while true; do echo -e 'HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, Modal!' | nc -l -p 8080; done"
                        .to_string(),
                ],
                encrypted_ports: vec![8080],
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

    // Wait a moment for the server to start
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Create a connect token for accessing the sandbox over HTTP
    let creds = client
        .sandboxes
        .create_connect_token(&sandbox.sandbox_id, None)
        .expect("Failed to create connect token");
    println!("Connect URL: {}", creds.url);
    println!("Token length: {} chars", creds.token.len());

    // Clean up
    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
