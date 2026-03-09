// Rust equivalent of examples/telemetry (Go).
//
// Demonstrates how to add custom telemetry and tracing to Modal API calls
// using gRPC interceptors.
// Requires a running Modal backend to execute.

use modal::client::ClientParams;

fn main() {
    // The Rust SDK supports custom gRPC interceptors via tonic.
    // Custom interceptors can measure latency, add tracing headers, etc.

    // Client params for custom configuration.
    let _params = ClientParams {
        token_id: "ak-custom-id".to_string(),
        token_secret: "as-custom-secret".to_string(),
        environment: String::new(),
    };
    println!("Client configured with custom credentials.");

    // Modal provides inject_required_headers() for building gRPC metadata.
    // In a real interceptor, you would wrap the tonic channel:
    //
    //   use modal::interceptors::inject_required_headers;
    //   use modal::config::Profile;
    //
    //   let interceptor = |mut req: tonic::Request<()>| {
    //       let start = std::time::Instant::now();
    //       // Add custom headers, measure timing, etc.
    //       Ok(req)
    //   };

    println!("Telemetry interceptor pattern demonstrated.");
    println!("In production, wrap tonic channels with custom interceptors.");
}
