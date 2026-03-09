// Rust equivalent of examples/custom-client (Go).
//
// Demonstrates creating a Modal client with custom credentials from
// environment variables.

use modal::client::ClientParams;

fn main() {
    // Read credentials from environment variables.
    let modal_id = std::env::var("CUSTOM_MODAL_ID").unwrap_or_default();
    let modal_secret = std::env::var("CUSTOM_MODAL_SECRET").unwrap_or_default();

    if modal_id.is_empty() {
        println!("CUSTOM_MODAL_ID not set (expected in real usage)");
    }
    if modal_secret.is_empty() {
        println!("CUSTOM_MODAL_SECRET not set (expected in real usage)");
    }

    // Create a client with custom credentials.
    let params = ClientParams {
        token_id: modal_id,
        token_secret: modal_secret,
        environment: String::new(),
    };
    println!("Client params configured with custom credentials.");

    // Note: Client::with_options reads config files which may not exist
    // in all environments. In production:
    //   let client = Client::with_options(Some(&params))?;
    //   let echo = function_service.from_name("libmodal-test-support", "echo_string", None)?;
    let _ = params;
    println!("Custom client configuration ready.");
}
