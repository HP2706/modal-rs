// Rust equivalent of examples/sandbox (Go).
//
// Demonstrates creating a Sandbox with stdin/stdout communication.
// Requires a running Modal backend to execute.

use modal::app::App;
use modal::image::Image;
use modal::sandbox::SandboxCreateParams;

fn main() {
    // In a real application, you would create a client and use gRPC services.
    // This example demonstrates the type construction patterns.

    let _app = App {
        app_id: "ap-example".to_string(),
        name: "libmodal-example".to_string(),
    };

    let image = Image {
        image_id: String::new(),
        image_registry_config: None,
        tag: "alpine:3.21".to_string(),
        layers: vec![],
    };
    println!("Image tag: {}", image.tag);

    let params = SandboxCreateParams {
        // Command: ["cat"] would be passed to the gRPC call
        ..Default::default()
    };
    println!("Sandbox params - PTY: {}, CPU: {}", params.pty, params.cpu);

    // With a real client, you would:
    // 1. Call sandbox_service.create(app, image, params) to create the sandbox
    // 2. Write to sandbox stdin
    // 3. Read from sandbox stdout
    // 4. Call sandbox.terminate() when done
    println!("Sandbox configuration ready.");
}
