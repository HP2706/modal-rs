// Rust equivalent of examples/sandbox-volume (Go).
//
// Demonstrates persistent Volume usage with write and read-only modes
// across multiple Sandboxes. Runs against real Modal API.

use std::collections::HashMap;

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::SandboxCreateParams;
use modal::volume::VolumeFromNameParams;

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

    // Build image
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
    println!("Image: {}", image.image_id);

    // Get or create a volume
    let volume = client
        .volumes
        .from_name(
            "libmodal-rs-example-volume",
            Some(&VolumeFromNameParams {
                create_if_missing: true,
                ..Default::default()
            }),
        )
        .expect("Failed to get or create volume");
    println!("Volume: {} ({})", volume.name, volume.volume_id);

    // Writer sandbox: mount volume read-write and write a file
    let writer_sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'Hello from modal-rs writer!' > /mnt/volume/message.txt && echo 'Wrote message.txt'"
                        .to_string(),
                ],
                volumes: HashMap::from([(
                    "/mnt/volume".to_string(),
                    volume.clone(),
                )]),
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create writer sandbox");
    println!("Writer sandbox: {}", writer_sandbox.sandbox_id);

    let result = client
        .sandboxes
        .wait(&writer_sandbox.sandbox_id, 60.0)
        .expect("Failed to wait for writer sandbox");
    println!(
        "Writer finished - exit_code: {}, success: {}",
        result.exit_code, result.success
    );

    // Reader sandbox: mount volume read-only and read the file
    let reader_sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "cat".to_string(),
                    "/mnt/volume/message.txt".to_string(),
                ],
                volumes: HashMap::from([(
                    "/mnt/volume".to_string(),
                    volume.read_only(),
                )]),
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create reader sandbox");
    println!("Reader sandbox: {}", reader_sandbox.sandbox_id);

    let result = client
        .sandboxes
        .wait(&reader_sandbox.sandbox_id, 60.0)
        .expect("Failed to wait for reader sandbox");
    println!(
        "Reader finished - exit_code: {}, success: {}",
        result.exit_code, result.success
    );

    // Try writing to a read-only volume (should fail)
    let fail_sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'This should fail' >> /mnt/volume/message.txt".to_string(),
                ],
                volumes: HashMap::from([(
                    "/mnt/volume".to_string(),
                    volume.read_only(),
                )]),
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create fail sandbox");
    println!("Read-only write attempt sandbox: {}", fail_sandbox.sandbox_id);

    let result = client
        .sandboxes
        .wait(&fail_sandbox.sandbox_id, 60.0)
        .expect("Failed to wait for fail sandbox");
    println!(
        "Read-only write attempt - exit_code: {}, success: {}",
        result.exit_code, result.success
    );

    // Clean up
    let _ = client.sandboxes.terminate(&writer_sandbox.sandbox_id);
    let _ = client.sandboxes.terminate(&reader_sandbox.sandbox_id);
    let _ = client.sandboxes.terminate(&fail_sandbox.sandbox_id);
    println!("Done!");
}
