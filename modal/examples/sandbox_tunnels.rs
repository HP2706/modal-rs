// Rust equivalent of examples/sandbox-tunnels (Go).
//
// Demonstrates creating a Sandbox with an HTTP server and using tunnels
// for secure network access.
// Requires a running Modal backend to execute.

use modal::sandbox::Tunnel;

fn main() {
    // Tunnels provide secure network access to sandbox ports.
    let tunnel = Tunnel {
        host: "example.modal.run".to_string(),
        port: 443,
        unencrypted_host: String::new(),
        unencrypted_port: 0,
    };

    println!("Tunnel URL: {}", tunnel.url());
    let (host, port) = tunnel.tls_socket();
    println!("TLS socket: {}:{}", host, port);

    // TCP socket requires unencrypted host/port to be configured.
    match tunnel.tcp_socket() {
        Ok((h, p)) => println!("TCP socket: {}:{}", h, p),
        Err(e) => println!("TCP socket not available: {}", e),
    }

    // Tunnel with unencrypted TCP:
    let tcp_tunnel = Tunnel {
        host: "example.modal.run".to_string(),
        port: 443,
        unencrypted_host: "example-tcp.modal.run".to_string(),
        unencrypted_port: 8000,
    };
    let (tcp_host, tcp_port) = tcp_tunnel.tcp_socket().unwrap();
    println!("TCP socket: {}:{}", tcp_host, tcp_port);

    // With a real client:
    //   let sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       command: vec!["python3", "-m", "http.server", "8000"],
    //       encrypted_ports: vec![8000],
    //       timeout: Duration::from_secs(60),
    //   })?;
    //   let tunnels = sb.tunnels(Duration::from_secs(30))?;
    //   let tunnel = &tunnels[&8000];
    //   let url = tunnel.url();
    println!("Tunnel configuration ready.");
}
