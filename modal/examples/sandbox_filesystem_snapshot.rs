// Demonstrates creating a filesystem snapshot and using it in a new Sandbox.
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

    // Create a sandbox and write some data
    let sb1 = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "mkdir -p /app/data && echo 'snapshot-data' > /app/data/info.txt && sleep 30"
                        .to_string(),
                ],
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox 1: {}", sb1.sandbox_id);

    // Wait a moment for the command to execute
    std::thread::sleep(std::time::Duration::from_secs(3));

    // Snapshot the filesystem
    println!("Taking filesystem snapshot...");
    let snapshot_id = client
        .sandboxes
        .snapshot_filesystem(&sb1.sandbox_id, 55.0)
        .expect("Failed to snapshot filesystem");
    println!("Snapshot image: {}", snapshot_id);

    let _ = client.sandboxes.terminate(&sb1.sandbox_id);

    // Create a new sandbox from the snapshot — the data should be there
    let sb2 = client
        .sandboxes
        .create(
            &app.app_id,
            &snapshot_id,
            SandboxCreateParams {
                command: vec!["cat".to_string(), "/app/data/info.txt".to_string()],
                timeout_secs: Some(60),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox from snapshot");
    println!("Sandbox 2 (from snapshot): {}", sb2.sandbox_id);

    let result = client
        .sandboxes
        .wait(&sb2.sandbox_id, 60.0)
        .expect("Failed to wait");
    println!(
        "exit_code={}, success={}",
        result.exit_code, result.success
    );

    // Clean up
    let _ = client.sandboxes.terminate(&sb2.sandbox_id);
    let _ = client.images.delete(&snapshot_id, None);
    println!("Done!");
}
