// Demonstrates looking up a Modal Cls with custom ServiceOptions.
// Runs against real Modal API.
//
// Requires: `modal deploy test_support.py` to deploy the test support app first.
//
// Note: The Rust SDK can resolve Cls definitions and configure ServiceOptions
// but does not yet implement with_options()/instance()/method() for invocation.

use std::collections::HashMap;

use modal::client::Client;
use modal::cls::ServiceOptions;

fn main() {
    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    // Create an ephemeral secret to inject as an env var
    let secret = client
        .secrets
        .from_map(
            &HashMap::from([("MY_SECRET_VAR".to_string(), "secret-value".to_string())]),
            None,
        )
        .expect("Failed to create ephemeral secret");
    println!("Ephemeral secret: {}", secret.secret_id);

    // Configure service options
    let _options = ServiceOptions {
        secrets: Some(vec![secret]),
        max_concurrent_inputs: Some(1),
        ..Default::default()
    };

    // Look up the deployed class
    let cls = client
        .cls
        .from_name("libmodal-rs-test-support", "EchoCls", None)
        .expect("Failed to look up Cls");
    println!("Cls service function ID: {}", cls.service_function_id);

    // TODO: Once with_options()/instance()/method() are implemented:
    //   let instance = cls.with_options(&options).instance(None)?;
    //   let method = instance.method("echo_env_var")?;
    //   let result = method.remote(transport, &args, &kwargs)?;

    println!("Done!");
}
