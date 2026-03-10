// Demonstrates reading and writing files in a Sandbox using filesystem operations.
// Runs against real Modal API.

use std::sync::Arc;

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::SandboxCreateParams;
use modal::sandbox_filesystem::{SandboxFilesystemService, SandboxFilesystemServiceImpl};

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

    // Create a long-running sandbox for filesystem operations
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec!["sleep".to_string(), "120".to_string()],
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

    // Get the task ID for filesystem operations
    let task_id = client
        .sandboxes
        .get_task_id(&sandbox.sandbox_id)
        .expect("Failed to get task ID");
    println!("Task ID: {}", task_id);

    // Create filesystem service using the transport
    let transport: Arc<modal::transport::ModalGrpcTransport> = client.transport().clone();
    let fs = SandboxFilesystemServiceImpl { client: transport };

    // Create a directory
    fs.mkdir(&task_id, "/tmp/example", true)
        .expect("Failed to create directory");
    println!("Created /tmp/example directory");

    // Write a file
    let mut file = fs
        .open(&task_id, "/tmp/example/hello.txt", "w")
        .expect("Failed to open file for writing");
    fs.write(&file, b"Hello from Rust SDK!\nLine two.\n")
        .expect("Failed to write file");
    fs.close(&mut file).expect("Failed to close file");
    println!("Wrote /tmp/example/hello.txt");

    // Read the file back
    let mut reader = fs
        .open(&task_id, "/tmp/example/hello.txt", "r")
        .expect("Failed to open file for reading");
    let content = fs.read(&reader, None).expect("Failed to read file");
    println!("Read back: {:?}", String::from_utf8_lossy(&content));
    fs.close(&mut reader).expect("Failed to close reader");

    // List directory
    let entries = fs.ls(&task_id, "/tmp/example").expect("Failed to list directory");
    println!("Directory listing: {:?}", entries);

    // Clean up the file
    fs.rm(&task_id, "/tmp/example/hello.txt", false)
        .expect("Failed to remove file");
    println!("Removed hello.txt");

    // Clean up
    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
