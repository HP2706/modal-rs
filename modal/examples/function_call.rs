// Demonstrates calling a deployed Modal Function from Rust.
//
// Requires deploying test_support.py first:
//   modal deploy test_support.py
//
// Then run:
//   cargo run --example function_call

use modal::client::Client;
use modal::invocation::NoBlobDownloader;

fn main() {
    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");
    println!("Connected (version: {})", client.version());

    // Look up the deployed echo_string function
    let echo = client
        .functions
        .from_name("libmodal-rs-test-support", "echo_string", None)
        .expect("Failed to get function (is test_support.py deployed?)");

    println!("Function: {}", echo.function_id);

    // Call with keyword args (matching Go example: echo.Remote(ctx, nil, map[string]any{"s": "Hello world!"}))
    let args = vec![];
    let kwargs = ciborium::Value::Map(vec![(
        ciborium::Value::Text("s".to_string()),
        ciborium::Value::Text("Hello from Rust!".to_string()),
    )]);

    println!("Calling echo_string(s=\"Hello from Rust!\")...");
    let transport = client.transport();
    let result = echo
        .remote(transport.as_ref(), &NoBlobDownloader, &args, &kwargs)
        .expect("Failed to call function");
    println!("Result: {:?}", result);
}
