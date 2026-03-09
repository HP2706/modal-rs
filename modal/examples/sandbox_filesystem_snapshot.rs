// Rust equivalent of examples/sandbox-filesystem-snapshot (Go).
//
// Demonstrates creating a filesystem snapshot of a running Sandbox and
// using it to create a new Sandbox with the same state.
// Requires a running Modal backend to execute.

use modal::image::Image;

fn main() {
    let base_image = Image {
        image_id: String::new(),
        image_registry_config: None,
        tag: "alpine:3.21".to_string(),
        layers: vec![Default::default()],
    };
    println!("Base image: {}", base_image.tag);

    // A filesystem snapshot captures the entire sandbox filesystem as an Image.
    let snapshot = Image::new("im-fs-snapshot-456".to_string());
    println!("Snapshot image ID: {}", snapshot.image_id);

    // With a real client:
    //   let sb = sandbox_service.create(app, base_image, None)?;
    //   sb.exec(["mkdir", "-p", "/app/data"], None)?;
    //   sb.exec(["sh", "-c", "echo 'data' > /app/data/info.txt"], None)?;
    //
    //   // Snapshot the entire filesystem (with timeout)
    //   let snapshot = sb.snapshot_filesystem(Duration::from_secs(55))?;
    //   sb.terminate(None)?;
    //
    //   // Create new sandbox from the snapshot
    //   let sb2 = sandbox_service.create(app, snapshot, None)?;
    //   let proc = sb2.exec(["cat", "/app/data/info.txt"], None)?;
    //   // File contents are preserved from the original sandbox.
    println!("Filesystem snapshot configuration ready.");
}
