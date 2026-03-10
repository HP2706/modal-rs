// Demonstrates creating a Modal client with custom credentials.
// Runs against real Modal API.
//
// Uses standard MODAL_TOKEN_ID and MODAL_TOKEN_SECRET env vars by default,
// or set CUSTOM_MODAL_ID and CUSTOM_MODAL_SECRET to use custom credentials.

use modal::client::{Client, ClientParams};

fn main() {
    // Try custom credentials first, fall back to default env vars
    let custom_id = std::env::var("CUSTOM_MODAL_ID").ok();
    let custom_secret = std::env::var("CUSTOM_MODAL_SECRET").ok();

    let client = if let (Some(id), Some(secret)) = (custom_id, custom_secret) {
        println!("Connecting with custom credentials...");
        Client::connect_with_options(Some(&ClientParams {
            token_id: id,
            token_secret: secret,
            environment: String::new(),
        }))
        .expect("Failed to connect with custom credentials")
    } else {
        println!("No custom credentials set, connecting with default...");
        println!("(Set CUSTOM_MODAL_ID and CUSTOM_MODAL_SECRET to test custom auth)");
        Client::connect().expect("Failed to connect")
    };

    println!("Client version: {}", client.sdk_version);
    println!("Environment: {}", client.profile.environment);

    // Verify the client works by listing or creating an app
    let _app = client
        .apps
        .from_name(
            "libmodal-rs-example",
            Some(&modal::app::AppFromNameParams {
                create_if_missing: true,
                ..Default::default()
            }),
        )
        .expect("Failed to create app — credentials may be invalid");
    println!("Successfully authenticated and created/found app.");

    println!("Done!");
}
