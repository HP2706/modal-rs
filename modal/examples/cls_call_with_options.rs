// Rust equivalent of examples/cls-call-with-options (Go).
//
// Demonstrates calling a Modal Cls with custom options including Secrets
// and concurrency configuration.
// Requires a running Modal backend to execute.

use modal::cls::ServiceOptions;
use modal::secret::Secret;

fn main() {
    // Create a Secret (normally via secret_service.from_map)
    let secret = Secret {
        secret_id: "st-secret-123".to_string(),
        name: String::new(),
    };
    println!("Secret ID: {}", secret.secret_id);

    // ServiceOptions can override Cls runtime configuration
    let options = ServiceOptions {
        secrets: Some(vec![secret]),
        max_concurrent_inputs: Some(1),
        ..Default::default()
    };
    println!(
        "Cls options - secrets: {}, max_concurrent_inputs: {:?}",
        options.secrets.as_ref().map_or(0, |s| s.len()),
        options.max_concurrent_inputs
    );

    // With a real client:
    //   let cls = cls_service.from_name("libmodal-test-support", "EchoClsParametrized", None)?;
    //   let instance_with_options = cls.with_options(&options).instance(None)?;
    //   let method = instance_with_options.method("echo_env_var")?;
    //   let result = method.remote(ctx, &["SECRET_MESSAGE"], None)?;
    println!("Cls with custom options configured.");
}
