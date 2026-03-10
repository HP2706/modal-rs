// Demonstrates telemetry and tracing patterns with the Modal client.
// Runs against real Modal API.

use modal::client::Client;

fn main() {
    println!("Connecting to Modal...");
    let start = std::time::Instant::now();
    let client = Client::connect().expect("Failed to connect to Modal");
    let connect_time = start.elapsed();
    println!("Connected in {:?}", connect_time);

    // Measure API call latency
    let start = std::time::Instant::now();
    let _app = client
        .apps
        .from_name(
            "libmodal-rs-example",
            Some(&modal::app::AppFromNameParams {
                create_if_missing: true,
                ..Default::default()
            }),
        )
        .expect("Failed to create app");
    let api_time = start.elapsed();
    println!("App lookup/create: {:?}", api_time);

    // The Rust SDK uses tonic gRPC with automatic retry and header injection.
    // Custom interceptors can be added by wrapping the transport channel.
    //
    // The interceptor chain (in modal::interceptors) handles:
    // - Required headers (x-modal-client-type, x-modal-token-id, etc.)
    // - Retry with backoff for transient errors
    // - SDK version reporting
    //
    // For custom telemetry, wrap calls with timing:
    let start = std::time::Instant::now();
    let image = client.images.from_registry("alpine:3.21", None);
    let _image = client.images.build(
        &image,
        &modal::image::ImageBuildParams {
            app_id: _app.app_id.clone(),
            ..Default::default()
        },
    ).expect("Failed to build image");
    let build_time = start.elapsed();
    println!("Image build: {:?}", build_time);

    println!("Done!");
}
