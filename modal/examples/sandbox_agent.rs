// Demonstrates running an AI agent in a Sandbox with PTY support and secrets.
// Runs against real Modal API.
//
// Requires:
// - A Modal Secret named "anthropic-secret" with ANTHROPIC_API_KEY
//
// Create the secret:
//   modal secret create anthropic-secret ANTHROPIC_API_KEY=sk-ant-...

use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::{SandboxCreateParams, SandboxExecParams};
use modal::secret::SecretFromNameParams;

fn main() {
    let secret_name =
        std::env::var("AGENT_SECRET_NAME").unwrap_or_else(|_| "anthropic-secret".to_string());

    println!("Connecting to Modal...");
    let client = Client::connect().expect("Failed to connect to Modal");

    let app = client
        .apps
        .from_name(
            "libmodal-rs-example",
            Some(&AppFromNameParams {
                create_if_missing: true,
                ..Default::default()
            }),
        )
        .expect("Failed to get or create app");

    // Look up the Anthropic API key secret
    let secret = client
        .secrets
        .from_name(&secret_name, Some(&SecretFromNameParams::default()))
        .expect("Failed to find secret — create it with `modal secret create`");

    // Build image with Claude CLI installed
    let base = client.images.from_registry("ubuntu:22.04", None);
    let image = base.dockerfile_commands(
        &[
            "RUN apt-get update && apt-get install -y --no-install-recommends bash curl git ripgrep && rm -rf /var/lib/apt/lists/*".to_string(),
            "RUN curl -fsSL https://claude.ai/install.sh | bash".to_string(),
            "ENV PATH=/root/.local/bin:$PATH USE_BUILTIN_RIPGREP=0".to_string(),
        ],
        None,
    );
    let image = client
        .images
        .build(
            &image,
            &ImageBuildParams {
                app_id: app.app_id.clone(),
                ..Default::default()
            },
        )
        .expect("Failed to build agent image");
    println!("Agent image: {}", image.image_id);

    // Create sandbox with the secret injected (secrets go on create, not exec)
    let sandbox = client
        .sandboxes
        .create(
            &app.app_id,
            &image.image_id,
            SandboxCreateParams {
                command: vec!["sleep".to_string(), "300".to_string()],
                secrets: vec![secret],
                timeout_secs: Some(300),
                ..Default::default()
            },
        )
        .expect("Failed to create sandbox");
    println!("Sandbox: {}", sandbox.sandbox_id);

    // Clone a repo
    let exec_id = client
        .sandboxes
        .exec(
            &sandbox,
            vec![
                "git".to_string(),
                "clone".to_string(),
                "--depth=1".to_string(),
                "https://github.com/anthropics/anthropic-cookbook.git".to_string(),
                "/repo".to_string(),
            ],
            Default::default(),
        )
        .expect("Failed to exec git clone");
    let result = client
        .sandboxes
        .exec_wait(&exec_id, 60.0)
        .expect("Failed to wait for git clone");
    println!("git clone: exit_code={:?}", result.exit_code);

    // Run Claude with PTY support (API key is available from sandbox secrets)
    let exec_id = client
        .sandboxes
        .exec(
            &sandbox,
            vec![
                "claude".to_string(),
                "-p".to_string(),
                "Summarize this repo in one sentence.".to_string(),
            ],
            SandboxExecParams {
                pty: true,
                workdir: "/repo".to_string(),
                ..Default::default()
            },
        )
        .expect("Failed to exec claude");

    let result = client
        .sandboxes
        .exec_wait(&exec_id, 120.0)
        .expect("Failed to wait for claude");
    println!("claude: exit_code={:?}", result.exit_code);

    let _ = client.sandboxes.terminate(&sandbox.sandbox_id);
    println!("Done!");
}
