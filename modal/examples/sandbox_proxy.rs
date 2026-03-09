// Rust equivalent of examples/sandbox-proxy (Go).
//
// Demonstrates creating a Sandbox with a configured Proxy for network requests.
// Requires a running Modal backend to execute.

use modal::proxy::{Proxy, ProxyFromNameParams};

fn main() {
    // Look up a proxy by name.
    let params = ProxyFromNameParams {
        environment: "libmodal".to_string(),
    };
    println!("Proxy lookup - environment: '{}'", params.environment);

    // A Proxy is resolved via ProxyService.from_name.
    let proxy = Proxy {
        proxy_id: "pr-proxy-123".to_string(),
    };
    println!("Proxy ID: {}", proxy.proxy_id);

    // With a real client:
    //   let proxy = proxy_service.from_name("libmodal-test-proxy", Some(&params))?;
    //   let sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       proxy: Some(proxy),
    //       ..Default::default()
    //   })?;
    //   let p = sb.exec(["curl", "-s", "ifconfig.me"], None)?;
    //   let ip = p.stdout.read_to_string()?;
    println!("Proxy-enabled sandbox configuration ready.");
}
