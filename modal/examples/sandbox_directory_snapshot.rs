// Demonstrates snapshotting a directory and mounting it in another Sandbox.
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

    // Create sandbox 1 and populate a directory
    let sb1 = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "mkdir -p /data && echo 'file-a' > /data/a.txt && echo 'file-b' > /data/b.txt && sleep 30"
                        .to_string(),
                ],
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox 1");
    println!("Sandbox 1: {}", sb1.sandbox_id);

    std::thread::sleep(std::time::Duration::from_secs(3));

    // Snapshot just the /data directory
    println!("Snapshotting /data directory...");
    let snapshot_id = client
        .sandboxes
        .snapshot_directory(&sb1, "/data")
        .expect("Failed to snapshot directory");
    println!("Directory snapshot image: {}", snapshot_id);

    let _ = client.sandboxes.terminate(&sb1.sandbox_id);

    // Create sandbox 2 and mount the snapshot
    let sb2 = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec![
                    "sh".to_string(),
                    "-c".to_string(),
                    "sleep 30".to_string(),
                ],
                timeout_secs: Some(120),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox 2");
    println!("Sandbox 2: {}", sb2.sandbox_id);

    // Mount the snapshot image into /data
    client
        .sandboxes
        .mount_image(&sb2, "/data", Some(&snapshot_id))
        .expect("Failed to mount image");
    println!("Mounted snapshot at /data");

    // Exec to verify files are there
    let exec_id = client
        .sandboxes
        .exec(
            &sb2,
            vec!["ls".to_string(), "/data".to_string()],
            Default::default(),
        )
        .expect("Failed to exec");

    let exec_result = client
        .sandboxes
        .exec_wait(&exec_id, 30.0)
        .expect("Failed to wait for exec");
    println!(
        "ls /data: completed={}, exit_code={:?}",
        exec_result.completed, exec_result.exit_code
    );

    // Clean up
    let _ = client.sandboxes.terminate(&sb2.sandbox_id);
    let _ = client.images.delete(&snapshot_id, None);
    println!("Done!");
}
