// Demonstrates using a temporary Volume with automatic cleanup.
// Runs against real Modal API.
//
// Note: True ephemeral volumes require a tokio runtime for the heartbeat task,
// which conflicts with the transport's own runtime. This example uses a named
// volume that is deleted at the end, achieving the same effect.

use std::collections::HashMap;

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::SandboxCreateParams;
use modal::volume::VolumeFromNameParams;

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

    // Create a temporary volume (will be deleted at the end)
    let vol_name = format!("libmodal-rs-temp-{}", std::process::id());
    let volume = client
        .volumes
        .from_name(
            &vol_name,
            Some(&VolumeFromNameParams {
                create_if_missing: true,
                ..Default::default()
            }),
        )
        .expect("Failed to create volume");
    println!("Volume: {} (name={})", volume.volume_id, vol_name);

    // Writer sandbox
    let writer = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "echo 'ephemeral data' > /mnt/vol/data.txt".to_string(),
                ],
                volumes: HashMap::from([("/mnt/vol".to_string(), volume.clone())]),
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create writer sandbox");

    let result = client
        .sandboxes
        .wait(&writer.sandbox_id, 60.0)
        .expect("Failed to wait");
    println!("Writer: exit_code={}", result.exit_code);

    // Reader sandbox (read-only)
    let reader = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec!["cat".to_string(), "/mnt/vol/data.txt".to_string()],
                volumes: HashMap::from([("/mnt/vol".to_string(), volume.read_only())]),
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create reader sandbox");

    let result = client
        .sandboxes
        .wait(&reader.sandbox_id, 60.0)
        .expect("Failed to wait");
    println!("Reader: exit_code={}", result.exit_code);

    // Clean up
    let _ = client.sandboxes.terminate(&writer.sandbox_id);
    let _ = client.sandboxes.terminate(&reader.sandbox_id);
    let _ = client.volumes.delete(&vol_name, None);
    println!("Volume cleaned up. Done!");
}
