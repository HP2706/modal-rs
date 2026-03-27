# modal-rs

`modal-rs` is a port of Modal's Go/JS SDK to Rust. Built autonomously by Claude.

The agent harness is inspired by anthropics work on getting Claude to autonomously build a C compiler from scratch [Building an efficient C compiler](https://www.anthropic.com/engineering/building-c-compiler). It is essentially just a for loop that spawns a Claude that looks in [PROGRESS.md](./PROGRESS.md), picks up a task, tests and commits it, and updates [PROGRESS.md](./PROGRESS.md) for the next agent.

## Status

The SDK is fully implemented, and all the tests that were in the go/js sdk pass. 
NOTE: I wouldnt recommend using this in production.
For more detail, see [ARCHITECTURE.md](./ARCHITECTURE.md) and [PROGRESS.md](./PROGRESS.md).

## Using sandboxes

The basic sandbox flow in this repo is:

1. Connect with `Client::connect()`.
2. Get or create an app with `client.apps.from_name(..., create_if_missing: true)`.
3. Build an image with `client.images.from_registry(...)` and `client.images.build(...)`.
4. Create a sandbox with `client.sandboxes.create(...)`.
5. Wait for completion with `client.sandboxes.wait(...)` or run extra commands with `exec(...)`.
6. Terminate the sandbox when you're done.

```rust
use modal::app::AppFromNameParams;
use modal::client::Client;
use modal::image::ImageBuildParams;
use modal::sandbox::{SandboxCreateParams, SandboxExecParams};

let client = Client::connect()?;

let app = client.apps.from_name(
    "libmodal-rs-example",
    Some(&AppFromNameParams {
        create_if_missing: true,
        ..Default::default()
    }),
)?;

let image = client.images.from_registry("alpine:3.21", None);
let image = client.images.build(
    &image,
    &ImageBuildParams {
        app_id: app.app_id.clone(),
        ..Default::default()
    },
)?;

let sandbox = client.sandboxes.create(
    &app.app_id,
    &image.image_id,
    SandboxCreateParams {
        timeout_secs: Some(60),
        ..Default::default()
    },
)?;

let exec_id = client.sandboxes.exec(
    &sandbox,
    vec!["echo".into(), "hello from modal-rs".into()],
    SandboxExecParams::default(),
)?;

let exec_result = client.sandboxes.exec_wait(&exec_id, 60.0)?;
client.sandboxes.terminate(&sandbox.sandbox_id)?;
```

This pattern is useful when you want to keep a sandbox alive and run one or more commands inside it with `client.sandboxes.exec(&sandbox, command, params)` followed by `client.sandboxes.exec_wait(...)`. See [modal/examples/sandbox.rs](./modal/examples/sandbox.rs), [modal/examples/sandbox_exec.rs](./modal/examples/sandbox_exec.rs), [modal/examples/sandbox_named.rs](./modal/examples/sandbox_named.rs), and [modal/examples/sandbox_tunnels.rs](./modal/examples/sandbox_tunnels.rs).

## Using volumes

Volumes are looked up or created through `client.volumes`, then mounted into a sandbox by path through `SandboxCreateParams.volumes`.

```rust
use std::collections::HashMap;

use modal::sandbox::{SandboxCreateParams, SandboxExecParams};
use modal::volume::VolumeFromNameParams;

let volume = client.volumes.from_name(
    "libmodal-rs-example-volume",
    Some(&VolumeFromNameParams {
        create_if_missing: true,
        ..Default::default()
    }),
)?;

let writer = client.sandboxes.create(
    &app.app_id,
    &image.image_id,
    SandboxCreateParams {
        volumes: HashMap::from([("/mnt/volume".to_string(), volume.clone())]),
        timeout_secs: Some(60),
        ..Default::default()
    },
)?;

let writer_exec = client.sandboxes.exec(
    &writer,
    vec![
        "sh".into(),
        "-c".into(),
        "echo 'hello' > /mnt/volume/message.txt".into(),
    ],
    SandboxExecParams::default(),
)?;

let writer_result = client.sandboxes.exec_wait(&writer_exec, 60.0)?;

let reader = client.sandboxes.create(
    &app.app_id,
    &image.image_id,
    SandboxCreateParams {
        volumes: HashMap::from([("/mnt/volume".to_string(), volume.read_only())]),
        timeout_secs: Some(60),
        ..Default::default()
    },
)?;

let reader_exec = client.sandboxes.exec(
    &reader,
    vec!["cat".into(), "/mnt/volume/message.txt".into()],
    SandboxExecParams::default(),
)?;

let reader_result = client.sandboxes.exec_wait(&reader_exec, 60.0)?;
```

Use `volume.read_only()` when you want a read-only mount. For cleanup, call `client.sandboxes.terminate(...)` for running sandboxes and `client.volumes.delete(name, None)` on a named volume when you no longer need it. See [modal/examples/sandbox_volume.rs](./modal/examples/sandbox_volume.rs) and [modal/examples/sandbox_volume_ephemeral.rs](./modal/examples/sandbox_volume_ephemeral.rs).
