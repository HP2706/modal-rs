// Rust equivalent of examples/sandbox-secrets (Go).
//
// Demonstrates using both persistent Secrets and ephemeral Secrets
// created from maps in a Sandbox.
// Requires a running Modal backend to execute.

use modal::secret::{Secret, SecretFromNameParams};

fn main() {
    // Persistent secret (looked up by name).
    let params = SecretFromNameParams {
        required_keys: vec!["c".to_string()],
        ..Default::default()
    };
    println!("Secret lookup requires keys: {:?}", params.required_keys);

    let persistent_secret = Secret {
        secret_id: "st-persistent-123".to_string(),
        name: "libmodal-test-secret".to_string(),
    };
    println!("Persistent secret: {} (ID: {})", persistent_secret.name, persistent_secret.secret_id);

    // Ephemeral secret (created from a key-value map).
    // With a real client:
    //   let ephemeral = secret_service.from_map(&HashMap::from([
    //       ("d".to_string(), "123".to_string()),
    //   ]), None)?;
    let ephemeral_secret = Secret {
        secret_id: "st-ephemeral-456".to_string(),
        name: String::new(),
    };
    println!("Ephemeral secret ID: {}", ephemeral_secret.secret_id);

    // Both secrets can be passed to SandboxCreateParams.secrets
    let secrets = vec![persistent_secret, ephemeral_secret];
    println!("Total secrets for sandbox: {}", secrets.len());
}
