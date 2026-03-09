// Rust equivalent of examples/sandbox-agent (Go).
//
// Demonstrates running an AI agent (Claude) in a Sandbox with PTY support,
// git cloning, and secret-based API key injection.
// Requires a running Modal backend to execute.

use modal::image::Image;
use modal::sandbox::SandboxExecParams;

fn main() {
    // Build image with Claude CLI installed.
    let base = Image {
        image_id: String::new(),
        image_registry_config: None,
        tag: "alpine:3.21".to_string(),
        layers: vec![Default::default()],
    };

    let image = base.dockerfile_commands(
        &[
            "RUN apk add --no-cache bash curl git libgcc libstdc++ ripgrep".to_string(),
            "RUN curl -fsSL https://claude.ai/install.sh | bash".to_string(),
            "ENV PATH=/root/.local/bin:$PATH USE_BUILTIN_RIPGREP=0".to_string(),
        ],
        None,
    );
    println!("Agent image layers: {}", image.layers.len());

    // PTY is required for interactive commands like Claude.
    let exec_params = SandboxExecParams {
        pty: true,
        workdir: "/repo".to_string(),
        ..Default::default()
    };
    println!("Exec params - PTY: {}, workdir: '{}'", exec_params.pty, exec_params.workdir);

    // With a real client:
    //   let sb = sandbox_service.create(app, image, None)?;
    //   let git = sb.exec(["git", "clone", repo_url, "/repo"], None)?;
    //   git.wait()?;
    //
    //   let secret = secret_service.from_name("libmodal-anthropic-secret", Some(&params))?;
    //   let claude = sb.exec(
    //       ["claude", "-p", "Summarize this repo."],
    //       Some(&SandboxExecParams {
    //           pty: true,
    //           secrets: vec![secret],
    //           workdir: "/repo".to_string(),
    //           ..Default::default()
    //       }),
    //   )?;
    //   claude.wait()?;
    //   let stdout = claude.stdout.read_to_string()?;
    println!("Agent sandbox configuration ready.");
}
