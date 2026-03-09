// Rust equivalent of examples/sandbox-connect-token (Go).
//
// Demonstrates creating Connect Tokens for secure HTTP access to a Sandbox.
// Requires a running Modal backend to execute.

use modal::sandbox::{SandboxCreateConnectCredentials, SandboxCreateConnectTokenParams};

fn main() {
    // Connect token params can include user metadata.
    let params = SandboxCreateConnectTokenParams {
        user_metadata: "abc".to_string(),
    };
    println!("Connect token metadata: '{}'", params.user_metadata);

    // Connect credentials contain a URL and bearer token.
    let creds = SandboxCreateConnectCredentials {
        url: "https://example.modal.run".to_string(),
        token: "tk-connect-token-123".to_string(),
    };
    println!("URL: {}", creds.url);
    println!("Token: {}", creds.token);

    // With a real client:
    //   // Server must listen on port 8080 for Connect Tokens.
    //   let sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       command: vec!["python3", "-m", "http.server", "8080"],
    //   })?;
    //   let creds = sb.create_connect_token(Some(&params))?;
    //   // Use creds.url with Authorization: Bearer {creds.token} header.
    println!("Connect token configuration ready.");
}
