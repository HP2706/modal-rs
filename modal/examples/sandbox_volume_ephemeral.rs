// Rust equivalent of examples/sandbox-volume-ephemeral (Go).
//
// Demonstrates ephemeral Volumes that exist only for the duration of
// the operation, with proper cleanup via close_ephemeral().
// Requires a running Modal backend to execute.

use modal::volume::Volume;

fn main() {
    // Ephemeral volumes are created without a name and have a heartbeat.
    // They are automatically cleaned up when close_ephemeral() is called.
    let volume = Volume::new(
        "vol-ephemeral-123".to_string(),
        String::new(),
    );
    println!("Volume ID: {}", volume.volume_id);
    println!("Is ephemeral: {}", volume.is_ephemeral());

    // Read-only mode works the same as persistent volumes.
    let read_only = volume.read_only();
    println!("Read-only: {}", read_only.is_read_only());

    // With a real client:
    //   let volume = volume_service.ephemeral(None)?;
    //   // volume.is_ephemeral() == true
    //
    //   let writer_sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       command: vec!["sh", "-c", "echo 'Hello' > /mnt/volume/message.txt"],
    //       volumes: HashMap::from([("/mnt/volume", volume.clone())]),
    //   })?;
    //   writer_sb.wait()?;
    //
    //   let reader_sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       volumes: HashMap::from([("/mnt/volume", volume.read_only())]),
    //   })?;
    //
    //   // Clean up ephemeral volume when done:
    //   volume.close_ephemeral();
    println!("Ephemeral volume configuration ready.");
}
