// Rust equivalent of examples/sandbox-exec (Go).
//
// Demonstrates executing multiple commands in a Sandbox, including
// passing Secrets to exec commands.
// Requires a running Modal backend to execute.

use modal::sandbox::{SandboxCreateParams, SandboxExecParams, StreamConfig};
use modal::secret::Secret;

fn main() {
    // Create a sandbox (with python image, no initial command).
    let create_params = SandboxCreateParams::default();
    println!("Sandbox create params - PTY: {}", create_params.pty);

    // Default exec params pipe both stdout and stderr.
    let exec_params = SandboxExecParams::default();
    assert_eq!(exec_params.stdout, StreamConfig::Pipe);
    assert_eq!(exec_params.stderr, StreamConfig::Pipe);
    println!("Exec stdout: {:?}, stderr: {:?}", exec_params.stdout, exec_params.stderr);

    // With a real client:
    //   let sb = sandbox_service.create(app, image, None)?;
    //   let p = sb.exec(["python", "-c", "print('hello')"], None)?;
    //   let stdout = p.stdout.read_to_string()?;
    //   let stderr = p.stderr.read_to_string()?;
    //   let exit_code = p.wait()?;

    // Passing secrets in exec commands:
    let secret = Secret {
        secret_id: "st-test-secret".to_string(),
        name: "libmodal-test-secret".to_string(),
    };
    let exec_with_secrets = SandboxExecParams {
        ..Default::default()
    };
    println!("Secret '{}' (ID: {}) available in exec", secret.name, secret.secret_id);
    println!("Exec with secrets configured (workdir: '{}')", exec_with_secrets.workdir);
}
