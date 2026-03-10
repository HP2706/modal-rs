// Demonstrates retrieving current statistics for a Modal Function.
// Runs against real Modal API.
//
// Requires: `modal deploy test_support.py` to deploy the test support app first.

use modal::client::Client;

fn main() {
    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    // Look up the deployed function
    let echo = client
        .functions
        .from_name("libmodal-rs-test-support", "echo_string", None)
        .expect("Failed to look up function");
    println!("Function ID: {}", echo.function_id);

    let transport = client.transport();

    // Get current stats for the function
    let stats = echo
        .get_current_stats(transport.as_ref())
        .expect("Failed to get function stats");
    println!("Function Statistics:");
    println!("  Backlog: {} inputs", stats.backlog);
    println!("  Total Runners: {} containers", stats.num_total_runners);

    println!("Done!");
}
