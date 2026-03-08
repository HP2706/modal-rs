/// Common test utilities for integration tests.
use modal::client::{Client, ClientParams};
use modal::error::ModalError;

/// Create a new test client from environment variables.
pub fn new_test_client() -> Result<Client, ModalError> {
    Client::with_options(Some(&ClientParams::default()))
}

/// Check if integration test credentials are available.
pub fn has_credentials() -> bool {
    std::env::var("MODAL_TOKEN_ID").is_ok() && std::env::var("MODAL_TOKEN_SECRET").is_ok()
}

/// Skip test if credentials are not available.
#[macro_export]
macro_rules! skip_if_no_credentials {
    () => {
        if !common::has_credentials() {
            eprintln!("Skipping: MODAL_TOKEN_ID not set");
            return;
        }
    };
}
