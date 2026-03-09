// Rust equivalent of examples/sandbox-poll (Go).
//
// Demonstrates using Poll() to check Sandbox status and Wait() to wait
// for completion with exit codes.
// Requires a running Modal backend to execute.

use modal::sandbox::{SandboxCreateParams, SandboxTerminateParams};

fn main() {
    // Create a sandbox that waits for input, then exits with a specific code.
    let _params = SandboxCreateParams {
        // Command: ["sh", "-c", "read line; exit 42"]
        ..Default::default()
    };
    println!("Sandbox params configured.");

    // Terminate params control whether to wait for sandbox to finish.
    let terminate_params = SandboxTerminateParams { wait: true };
    println!("Terminate will wait: {}", terminate_params.wait);

    // With a real client:
    //   let sb = sandbox_service.create(app, image, &params)?;
    //   let poll_result = sb.poll()?;       // None while running
    //   sb.stdin.write(b"hello\n")?;
    //   sb.stdin.close()?;
    //   let exit_code = sb.wait()?;         // blocks until done
    //   let final_poll = sb.poll()?;        // Some(42)
    println!("Poll/wait configuration ready.");
}
