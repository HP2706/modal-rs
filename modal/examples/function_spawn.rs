// Demonstrates spawning a Modal Function asynchronously and calling it.
// Runs against real Modal API.
//
// Requires: `modal deploy test_support.py` to deploy the test support app first.

use modal::client::Client;
use modal::invocation::NoBlobDownloader;

fn main() {
    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    // Look up the deployed function
    let echo = client
        .functions
        .from_name("libmodal-rs-test-support", "echo_string", None)
        .expect("Failed to look up function");
    println!("Function ID: {}", echo.function_id);

    // Build arguments: echo_string(s="Hello from spawn!")
    let args = vec![];
    let kwargs = ciborium::Value::Map(vec![(
        ciborium::Value::Text("s".to_string()),
        ciborium::Value::Text("Hello from spawn!".to_string()),
    )]);

    let transport = client.transport();

    // Spawn the function asynchronously (returns immediately with a call ID)
    let call_id = echo
        .spawn(transport.as_ref(), &args, &kwargs)
        .expect("Failed to spawn function");
    println!("Spawned function call: {}", call_id);

    // Note: FunctionCall.get() is not yet implemented in the Rust SDK,
    // so we can't retrieve the spawned result. Demonstrate remote() instead.
    let result = echo
        .remote(transport.as_ref(), &NoBlobDownloader, &args, &kwargs)
        .expect("Failed to call function");
    println!("Remote result: {:?}", result);

    println!("Done!");
}
