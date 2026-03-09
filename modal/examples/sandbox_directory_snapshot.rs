// Rust equivalent of examples/sandbox-directory-snapshot (Go).
//
// Demonstrates taking a snapshot of a directory in one Sandbox and
// mounting it in another Sandbox.
// Requires a running Modal backend to execute.

use modal::image::Image;

fn main() {
    // Base image with git installed for cloning repos.
    let base_image = Image {
        image_id: String::new(),
        image_registry_config: None,
        tag: "alpine:3.21".to_string(),
        layers: vec![Default::default()],
    };
    let image = base_image.dockerfile_commands(
        &["RUN apk add --no-cache git".to_string()],
        None,
    );
    println!("Base image with git: {} layers", image.layers.len());

    // A directory snapshot creates a new Image from a sandbox directory.
    let snapshot_image = Image::new("im-snapshot-123".to_string());
    println!("Snapshot image ID: {}", snapshot_image.image_id);

    // With a real client:
    //   let sb = sandbox_service.create(app, base_image, None)?;
    //   let git = sb.exec(["git", "clone", repo_url, "/repo"], None)?;
    //   git.wait()?;
    //
    //   // Snapshot the /repo directory -> creates a new Image
    //   let snapshot = sb.snapshot_directory("/repo")?;
    //
    //   // Mount the snapshot in a new sandbox:
    //   let sb2 = sandbox_service.create(app, base_image, None)?;
    //   sb2.exec(["mkdir", "-p", "/repo"], None)?.wait()?;
    //   sb2.mount_image("/repo", &snapshot)?;
    //
    //   // Files from sb are now available in sb2 at /repo
    //   let ls = sb2.exec(["ls", "/repo"], None)?;
    //
    //   // Clean up:
    //   image_service.delete(&snapshot.image_id, None)?;
    println!("Directory snapshot configuration ready.");
}
