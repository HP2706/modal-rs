// Rust equivalent of examples/sandbox-filesystem (Go).
//
// Demonstrates reading and writing files in a Sandbox using filesystem operations.
// Requires a running Modal backend to execute.

use modal::sandbox_filesystem::{FileMode, SeekWhence, SystemErrorCode};

fn main() {
    // FileMode parsing for Unix-style file modes.
    let write_mode = FileMode::parse("w").unwrap();
    println!("Write mode: {:?}", write_mode);

    let read_mode = FileMode::parse("r").unwrap();
    println!("Read mode: {:?}", read_mode);

    let append_mode = FileMode::parse("a").unwrap();
    println!("Append mode: {:?}", append_mode);

    // SeekWhence for file positioning.
    let seek_set = SeekWhence::Set;
    let seek_cur = SeekWhence::Current;
    let seek_end = SeekWhence::End;
    println!("Seek positions: Set={:?}, Current={:?}, End={:?}", seek_set, seek_cur, seek_end);

    // SystemErrorCode maps sandbox errno values to error types.
    let not_found = SystemErrorCode::NotFound;
    println!("NotFound error: {}", not_found.description());

    let perm_denied = SystemErrorCode::PermissionDenied;
    println!("PermissionDenied error: {}", perm_denied.description());

    // With a real client:
    //   let write_file = sb.open("/tmp/example.txt", "w")?;
    //   write_file.write(b"Hello, Modal filesystem!\n")?;
    //   write_file.close()?;
    //
    //   let reader = sb.open("/tmp/example.txt", "r")?;
    //   let content = reader.read_all()?;
    //   reader.close()?;
    println!("Filesystem operations configured.");
}
