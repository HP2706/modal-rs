// Demonstrates creating a Sandbox with tunnels for secure network access.
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

    // Start an HTTP server on port 8000 with encrypted port access
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "while true; do echo -e 'HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, Modal!' | nc -l -p 8000; done"
                        .to_string(),
                ],
                encrypted_ports: vec![8000],
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

    // Get tunnel URLs (waits for tunnels to be provisioned)
    println!("Waiting for tunnels...");
    let tunnels = client
        .sandboxes
        .tunnels(&sandbox.sandbox_id, 30.0)
        .expect("Failed to get tunnels");

    for (port, tunnel) in &tunnels {
        println!("Port {}: URL = {}", port, tunnel.url());
        let (host, tls_port) = tunnel.tls_socket();
        println!("  TLS socket: {}:{}", host, tls_port);
        match tunnel.tcp_socket() {
            Ok((h, p)) => println!("  TCP socket: {}:{}", h, p),
            Err(_) => println!("  TCP socket: not available"),
        }
    }

    // Clean up
    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
