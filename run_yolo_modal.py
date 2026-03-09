"""Modal app with Rust + Claude Code using Sandbox snapshots for persistent auth."""

import json
import os
from pathlib import Path

import modal
from modal.stream_type import StreamType

app = modal.App("claude-rust-dev")

REPO_URL = "https://github.com/HP2706/modal-rs"
github_secret = modal.Secret.from_name("github-token")
REPO_DIR = "/root/modal-rs"
CONFIG_FILE = Path(__file__).parent / "sandbox_config.json"

# Base image: Rust + Claude Code (no auth)
base_image = (
    modal.Image.debian_slim()
    .apt_install("curl", "build-essential", "git", "pkg-config", "libssl-dev", "openssh-server")
    .run_commands("curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y")
    .env({
        "PATH": "/root/.cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin",
        "PYTHONUNBUFFERED": "1",
    })
    .run_commands(
        "curl -fsSL https://deb.nodesource.com/setup_22.x | bash -",
        "apt-get install -y nodejs",
        "npm install -g @anthropic-ai/claude-code",
    )
    .uv_pip_install("modal")
    .add_local_file(
        local_path='sandbox_config.json',
        remote_path='/root/sandbox_config.json',
    )
)

repo_vol = modal.Volume.from_name("modal-rs-repo", create_if_missing=True)


SSH_PORT = 8022

SSH_SETUP_SCRIPT = """
set -euo pipefail
passwd -d root || true
mkdir -p /run/sshd
cat > /etc/ssh/sshd_config.d/custom.conf << 'EOF'
PrintMotd no
PrintLastLog no
UsePAM no
PermitRootLogin yes
PasswordAuthentication yes
KbdInteractiveAuthentication no
ChallengeResponseAuthentication no
PermitEmptyPasswords yes
PubkeyAuthentication no
AuthorizedKeysFile none
Port 8022
EOF
sed -i 's/^#\\?PermitRootLogin.*/PermitRootLogin yes/' /etc/ssh/sshd_config || true
sed -i 's/^#\\?UsePAM.*/UsePAM no/' /etc/ssh/sshd_config || true
/usr/sbin/sshd
sleep 1
pgrep -x sshd >/dev/null || (echo "ERROR: sshd failed to start" && exit 1)
echo "SSH server started on port 8022"
"""


def _load_image() -> modal.Image:
    if os.path.exists('sandbox_config.json'):
        with open('sandbox_config.json') as f:
            image_id = json.load(f)['image_id']
        return modal.Image.from_id(image_id)
    return base_image



def _create_sandbox(image_id: str | None = None) -> modal.Sandbox:
    """Create a sandbox with the configured image, volume, and secrets."""
    if image_id is None:
        image = _load_image()
    else:
        image = modal.Image.from_id(image_id)
        
    return modal.Sandbox.create(
        app=app,
        image=image.env({"IS_SANDBOX": "1"}),
        secrets=[github_secret],
        volumes={REPO_DIR: repo_vol},
        timeout=60*60*24,
    )


def _setup_git(sb: modal.Sandbox) -> None:
    """Configure git auth inside a sandbox."""
    token = os.environ["GITHUB_TOKEN"]
    sb.exec(
        "git", "config", "--global",
        f"url.https://{token}@github.com/.insteadOf", "https://github.com/",
    ).wait()
    sb.exec("git", "config", "--global", "--add", "safe.directory", "*").wait()


def _pull_latest(sb: modal.Sandbox) -> None:
    """Pull latest changes in the repo volume."""
    sb.exec("git", "fetch", "origin", workdir=REPO_DIR).wait()
    rebase = sb.exec("git", "rebase", "-X", "theirs", "origin/main", workdir=REPO_DIR)
    rebase.wait()
    if rebase.returncode != 0:
        sb.exec("git", "rebase", "--abort", workdir=REPO_DIR).wait()
        sb.exec("git", "reset", "--hard", "origin/main", workdir=REPO_DIR).wait()
    sb.exec("git", "submodule", "update", "--init", "--recursive", workdir=REPO_DIR).wait()


def _run_claude(sb: modal.Sandbox, prompt: str, logfile: str, max_turns: int | None = None) -> int:
    """Run claude in the sandbox with stream-json output, stream to local stdout, write logfile in sandbox."""
    import re
    import sys

    max_turns_flag = f"--max-turns {max_turns} " if max_turns else ""
    # No | tee — claude writes directly to the PTY so Node.js doesn't buffer.
    # We collect raw output, extract JSON lines, and write the logfile ourselves.
    cmd = (
        f"cd {REPO_DIR} && claude -p --dangerously-skip-permissions "
        f"--verbose {max_turns_flag}"
        f"--output-format stream-json "
        f"--model claude-opus-4-6 "
        f"{repr(prompt)}"
    )

    proc = sb.exec(
        "bash", "-c",
        cmd,
        pty=True,  # MUST be true otherwise claude code breaks.
        stdout=StreamType.PIPE,
        stderr=StreamType.PIPE,
    )

    print(f'cmd: {cmd}', flush=True)

    # Start a log writer process in the sandbox — we'll pipe JSONL lines
    # to it in real-time. Uses >> append per line so each write is flushed
    # to disk immediately (important for volume commits to capture data).
    log_writer = sb.exec(
        "bash", "-c",
        f"while IFS= read -r line; do printf '%s\\n' \"$line\" >> {logfile}; done",
    )

    # PTY output contains ANSI escape codes mixed with JSON lines.
    # Strip escape codes, extract JSON lines, print + write each immediately.
    ansi_re = re.compile(r'\x1b[\[\]()][0-9;?]*[a-zA-Z\x07]|\r')
    line_buf = ""
    stdout_iter = iter(proc.stdout)
    while True:
        try:
            chunk = next(stdout_iter)
        except StopIteration:
            break
        except UnicodeDecodeError:
            # Modal splits raw bytes at arbitrary boundaries which can
            # bisect multi-byte UTF-8 chars (e.g. 0xe2 for '…').
            # Skip the malformed chunk — at worst we lose one character.
            continue
        clean = ansi_re.sub('', chunk)
        line_buf += clean
        while '\n' in line_buf:
            line, line_buf = line_buf.split('\n', 1)
            line = line.strip()
            if line.startswith('{'):
                print(line, flush=True)
                log_writer.stdin.write(line + '\n')
                log_writer.stdin.drain()
    # Flush remaining buffer
    remaining = line_buf.strip()
    if remaining.startswith('{'):
        print(remaining, flush=True)
        log_writer.stdin.write(remaining + '\n')
        log_writer.stdin.drain()
    proc.wait()

    # Close the log writer
    log_writer.stdin.write_eof()
    log_writer.stdin.drain()
    log_writer.wait()

    return proc.returncode

@app.local_entrypoint()
def setup_auth():
    """Create a sandbox with SSH, let user configure claude, then snapshot.

    Usage:
        modal run run_yolo_modal.py::setup_auth
        # SSH in using the printed command
        # Run `claude` to complete onboarding/auth
        # Exit SSH, press Enter here to snapshot
    """
    import time

    with modal.enable_output():
        sb = modal.Sandbox.create(
            app=app,
            image=base_image,
            timeout=3600,
            unencrypted_ports=[SSH_PORT],
        )

    print(f"\nSandbox created: {sb.object_id}")
    print("Starting SSH server...")

    ssh_setup = sb.exec("bash", "-c", SSH_SETUP_SCRIPT)
    ssh_setup.wait()
    print(ssh_setup.stdout.read())

    time.sleep(1)

    tunnel_info = sb.tunnels()
    tunnel = tunnel_info[SSH_PORT]
    host, port = tunnel.tcp_socket

    ssh_cmd = (
        f"ssh -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null "
        f"-o PreferredAuthentications=password -o PubkeyAuthentication=no "
        f"-o NumberOfPasswordPrompts=1 -p {port} root@{host}"
    )

    print(f"\nSSH into the sandbox with:")
    print(f"  {ssh_cmd}")
    print(f"\n(password is empty — just press Enter)")
    print(f"\nInside, run `claude` to complete onboarding/auth.")
    print(f"Then exit SSH and come back here.")
    input("\nPress Enter when done to snapshot...")

    print("Snapshotting sandbox filesystem...")
    snapshot_image = sb.snapshot_filesystem()
    image_id = snapshot_image.object_id

    CONFIG_FILE.write_text(json.dumps({"image_id": image_id}, indent=2))
    print(f"Saved image ID to {CONFIG_FILE}: {image_id}")
    print("Now run: python run_yolo_modal.py")

    sb.terminate()
    
    
@app.function(
    cpu=1,
    memory=4096,
    timeout=60*60*24,
    image=base_image,
    secrets=[github_secret],
    volumes={REPO_DIR: repo_vol},
)
def shell():
    pass

# --- Main entrypoints (all sandbox-based) ---

def setup_repo(image_id: str | None = None):
    """Clone/update the repo in the volume."""
    sb = _create_sandbox(image_id)
    _setup_git(sb)

    result = sb.exec("test", "-d", f"{REPO_DIR}/.git")
    result.wait()

    if result.returncode == 0:
        print("Repo exists, pulling latest...")
        _pull_latest(sb)
    else:
        token = os.environ["GITHUB_TOKEN"]
        auth_url = REPO_URL.replace("https://", f"https://{token}@")
        print(f"Cloning {REPO_URL}...")
        sb.exec("git", "clone", auth_url, REPO_DIR).wait()
        sb.exec("git", "submodule", "update", "--init", "--recursive", workdir=REPO_DIR).wait()

    log = sb.exec("git", "log", "--oneline", "-5", workdir=REPO_DIR)
    log.wait()
    print(f"Recent commits:\n{log.stdout.read()}")
    sb.terminate()


def run_yolo(image_id: str | None = None):
    """Pull latest then run the YOLO dev loop."""
    from datetime import datetime

    sb = _create_sandbox(image_id)
    _setup_git(sb)

    result = sb.exec("test", "-d", f"{REPO_DIR}/.git")
    result.wait()
    if result.returncode != 0:
        print("ERROR: Repo not set up. Run setup_repo() first.")
        sb.terminate()
        return

    print("Pulling latest changes...", flush=True)
    _pull_latest(sb)

    run_name = f"run_{datetime.now().strftime('%Y-%m-%d-%H-%M-%S')}"
    run_dir = f"{REPO_DIR}/agent_logs/{run_name}"
    sb.exec("mkdir", "-p", run_dir).wait()

    # Read the agent prompt
    cat_proc = sb.exec("cat", f"{REPO_DIR}/AGENT_PROMPT.md")
    cat_proc.wait()
    prompt = cat_proc.stdout.read()

    commit_proc = sb.exec("git", "rev-parse", "--short=6", "HEAD", workdir=REPO_DIR)
    commit_proc.wait()
    commit = commit_proc.stdout.read().strip()

    print(f"Run folder: {run_dir}", flush=True)

    loop_n = 20
    for i in range(1, loop_n + 1):
        logfile = f"{run_dir}/agent_{commit}_{i}.jsonl"
        print(f"\n{'='*60}", flush=True)
        print(f"[Loop {i}/{loop_n}] Starting claude agent...", flush=True)
        print(f"{'='*60}", flush=True)

        rc = _run_claude(sb, prompt, logfile)

        print(f"\n[Loop {i}/{loop_n}] Exit code: {rc}", flush=True)

        commit_proc = sb.exec("git", "rev-parse", "--short=6", "HEAD", workdir=REPO_DIR)
        commit_proc.wait()
        commit = commit_proc.stdout.read().strip()

    sb.terminate()
    print("Done.", flush=True)


def test_logging(image_id: str | None = None):
    """Test that claude output is forwarded to stdout and written to logfile."""
    sb = _create_sandbox(image_id)
    _setup_git(sb)

    result = sb.exec("test", "-d", f"{REPO_DIR}/.git")
    result.wait()
    if result.returncode != 0:
        print("ERROR: Repo not set up. Run setup_repo() first.")
        sb.terminate()
        return

    sb.exec("mkdir", "-p", f"{REPO_DIR}/agent_logs").wait()
    logfile = f"{REPO_DIR}/agent_logs/test_logging.jsonl"

    print(f"Logfile: {logfile}", flush=True)
    print("Starting claude...", flush=True)

    rc = _run_claude(
        sb,
        "Summarize this repo in 2-3 sentences. Be concise.",
        logfile,
        max_turns=2,
    )

    print(f"\n{'='*60}", flush=True)
    print(f"Exit code: {rc}", flush=True)

    # Read and print the logfile
    log_proc = sb.exec("wc", "-c", logfile)
    log_proc.wait()
    print(f"Logfile: {log_proc.stdout.read().strip()}", flush=True)

    sb.terminate()
    
    
def main(
    cmd : str,
    detach : bool = False,
):
    print("detaching:", detach)
    if cmd == "setup_repo":
        fn = setup_repo
    elif cmd == "run_yolo":
        fn = run_yolo
    elif cmd == "test_logging":
        fn = test_logging
    else:
        raise ValueError(f"Unknown command: {cmd}")
    
    modal_fn = app.function(
        volumes={REPO_DIR: repo_vol}, 
        timeout=60*60*24, 
        image=base_image,
        secrets=[github_secret],
    )(fn)
    
    with open(CONFIG_FILE, 'r') as f:
        image_id = json.load(f)['image_id']
    with modal.enable_output(), app.run(detach=detach):
        result = modal_fn.remote(image_id=image_id)
        print("result:", result)
    
if __name__ == "__main__":
    import fire
    fire.Fire(main)