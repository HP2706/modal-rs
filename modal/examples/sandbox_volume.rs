// Rust equivalent of examples/sandbox-volume (Go).
//
// Demonstrates persistent Volume usage with write and read-only modes
// across multiple Sandboxes.
// Requires a running Modal backend to execute.

use modal::volume::{Volume, VolumeFromNameParams};

fn main() {
    // Look up or create a named volume.
    let params = VolumeFromNameParams {
        create_if_missing: true,
        ..Default::default()
    };
    println!("Volume params - create_if_missing: {}", params.create_if_missing);

    // Volume in read-write mode.
    let volume = Volume::new(
        "vol-example-123".to_string(),
        "libmodal-example-volume".to_string(),
    );
    println!("Volume: {} (ID: {})", volume.name, volume.volume_id);
    println!("Read-only: {}", volume.is_read_only());

    // Volume in read-only mode (for reader sandboxes).
    let read_only_volume = volume.read_only();
    println!("Read-only copy: {}", read_only_volume.is_read_only());

    // With a real client:
    //   let volume = volume_service.from_name("libmodal-example-volume", Some(&params))?;
    //
    //   // Writer sandbox mounts volume read-write:
    //   let writer_sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       command: vec!["sh", "-c", "echo 'Hello' > /mnt/volume/message.txt"],
    //       volumes: HashMap::from([("/mnt/volume", volume.clone())]),
    //   })?;
    //   writer_sb.wait()?;
    //
    //   // Reader sandbox mounts volume read-only:
    //   let reader_sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       volumes: HashMap::from([("/mnt/volume", volume.read_only())]),
    //   })?;
    println!("Volume configuration ready.");
}
