// Demonstrates looking up a Modal Cls (class) by name.
// Runs against real Modal API.
//
// Requires: `modal deploy test_support.py` to deploy the test support app first.
//
// Note: The Rust SDK can resolve Cls definitions via from_name() but does not yet
// implement instance()/method()/remote() for invoking class methods. This example
// demonstrates the lookup portion.

use modal::client::Client;

fn main() {
    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    // Look up the deployed class
    let cls = client
        .cls
        .from_name("libmodal-rs-test-support", "EchoCls", None)
        .expect("Failed to look up Cls");
    println!("Cls service function ID: {}", cls.service_function_id);

    if let Some(ref metadata) = cls.service_function_metadata {
        println!(
            "Function name: {:?}",
            metadata.function_name
        );
    }

    // TODO: Once instance()/method()/remote() are implemented:
    //   let instance = cls.instance(None)?;
    //   let method = instance.method("echo_string")?;
    //   let result = method.remote(transport, &args, &kwargs)?;

    println!("Done!");
}
