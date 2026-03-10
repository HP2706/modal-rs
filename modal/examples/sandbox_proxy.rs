// Demonstrates looking up a Modal Proxy by name.
// Runs against real Modal API.
//
// Requires:
// - A pre-configured Modal Proxy (set up via Modal dashboard)
//
// Note: The Rust SDK can look up proxies via from_name() but SandboxCreateParams
// does not yet support the proxy field. This example demonstrates the lookup.

use modal::client::Client;
use modal::proxy::ProxyFromNameParams;

fn main() {
    let proxy_name =
        std::env::var("PROXY_NAME").unwrap_or_else(|_| "libmodal-test-proxy".to_string());

    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    // Look up the proxy
    let proxy = client
        .proxies
        .from_name(
            &proxy_name,
            Some(&ProxyFromNameParams {
                environment: String::new(),
            }),
        )
        .expect("Failed to find proxy");
    println!("Proxy ID: {}", proxy.proxy_id);

    // TODO: Once SandboxCreateParams supports proxy:
    //   let sandbox = client.sandboxes.create(&app_id, &image_id, SandboxCreateParams {
    //       proxy: Some(proxy),
    //       command: vec!["wget", "-qO-", "ifconfig.me"],
    //       ..Default::default()
    //   })?;

    println!("Done!");
}
