# Modal Rust SDK Progress

## 2026-03-09 — Ephemeral module implementation

### What was done
Created dedicated `ephemeral.rs` module (`modal/src/ephemeral.rs`) matching Go SDK's `ephemeral.go`:
- **`start_ephemeral_heartbeat`**: Shared function that spawns a tokio task running a heartbeat at 300s intervals, cancellable via `Notify`
- **`EPHEMERAL_OBJECT_HEARTBEAT_SLEEP`**: Shared constant (300s)
- Refactored `volume.rs` to use shared module instead of inline implementation
- Fixed `queue.rs` `ephemeral()` method which was missing the heartbeat start call
- 3 new unit tests (cancel, fires on interval, ignores errors)

### Test counts
- Before: 329 unit tests
- After: 332 unit tests (3 new)
- All passing

### What's next (priority order)
1. **Sandbox I/O streaming and ContainerProcess** — Stdin/Stdout/Stderr, lazy stream readers, ContainerProcess type (F017 in_progress)
2. **Integration tests** — All 17 test files in `modal/tests/` are empty (F023)
3. **Examples** — 26 Go examples need Rust equivalents (F024)

---

## 2026-03-09 — Sandbox module completion

### What was done
Extended the Sandbox module (`modal/src/sandbox.rs`) with all missing Go SDK methods:
- **Service methods**: FromID, FromName (with NotFound mapping), List (paginated), Poll
- **Tag management**: SetTags, GetTags
- **Tunnel support**: Tunnels (with timeout), Tunnel type with url()/tls_socket()/tcp_socket()
- **Filesystem ops**: SnapshotFilesystem, SnapshotDirectory, MountImage (with empty dir support)
- **Connect tokens**: CreateConnectToken with SandboxCreateConnectCredentials
- **Helper types**: GenericResultStatus, get_return_code, SandboxPollResult, SandboxListEntry, etc.
- 34 new sandbox tests (329 total)

### Test counts
- Before: 295 unit tests
- After: 329 unit tests (34 new)
- All passing

### What's next (priority order)
1. **Sandbox I/O streaming and ContainerProcess** — Stdin/Stdout/Stderr, lazy stream readers, ContainerProcess type
2. **Queue Put/Get/Iterate** — Need pickle serialization infrastructure
3. **Ephemeral module** — Heartbeat/ephemeral object support (ephemeral.go)
4. **Integration tests** — All 17 test files in `modal/tests/` are empty
5. **Examples** — 26 Go examples need Rust equivalents

---

## 2026-03-09 — Image.Build() layer-by-layer implementation

### What was done
Refactored the Image module (`modal/src/image.rs`) to implement proper layer-by-layer Build() matching the Go SDK:
- **Layer-by-layer build orchestration**: iterates through layers sequentially, each getting its own `ImageGetOrCreate` RPC
- **FROM tag construction**: first layer uses `FROM <tag>`, subsequent layers use `FROM base` with `BaseImage` linking to previous built image ID
- **Streaming polling**: `ImageJoinStreaming` with resumable `last_entry_id` for builds in progress
- **New types**: `ImageLayerBuildRequest`, `BaseImage`, `ImageJoinStreamingResult`, `ImageBuildStatus::Timeout/Terminated`
- **Pre-build validation**: validates all layers' dockerfile commands before making any RPCs
- **GPU config, secrets, force_build** propagation per-layer
- **build() returns full Image** (not just ID) preserving metadata (tag, registry config, layers)
- **Request recording** in mock for assertion of exact request parameters per layer
- 37 new image tests (295 total), including multi-layer orchestration test matching Go's `TestDockerfileCommandsWithOptions`

### Test counts
- Before: 258 unit tests
- After: 295 unit tests (37 new)
- All passing

### What's next (priority order)
1. **Sandbox module completion** — Missing FromID, FromName, List, Exec details, Wait, Tunnels, ContainerProcess, I/O streaming
2. **Queue Put/Get/Iterate** — Need pickle serialization infrastructure
3. **Integration tests** — All 17 test files in `modal/tests/` are empty
4. **Examples** — 26 Go examples need Rust equivalents

---

## 2026-03-09 — Function module implementation

### What was done
Implemented the Function module (`modal/src/function.rs`) with:
- **FunctionService** trait and **FunctionServiceImpl** with `from_name` (Cls method detection, environment parameter)
- **Function** struct with handle_metadata, `create_input` (CBOR encoding), web URL validation
- **Remote**: execute function with automatic retry on InternalFailure (up to 8 retries)
- **Spawn**: start function execution asynchronously, returns function call ID
- **GetCurrentStats**: retrieve backlog and runner count
- **UpdateAutoscaler**: override min/max/buffer containers and scaledown window
- **FunctionGrpcClient** trait for testability
- 29 unit tests covering all operations and edge cases

### Test counts
- Before: 229 unit tests
- After: 258 unit tests (29 new)
- All passing

### What's next (priority order)
1. **Image.Build()** — Streaming gRPC build orchestration (structural types done, build method not yet implemented).
2. **Queue Put/Get/Iterate** — Need pickle serialization infrastructure.
3. **Integration tests** — All 17 test files in `modal/tests/` are empty.
4. **Examples** — 26 Go examples need Rust equivalents.

---

## 2026-03-09 — Invocation module implementation

### What was done
Implemented the Invocation module (`modal/src/invocation.rs`) with:
- **InvocationGrpcClient** trait abstracting all gRPC calls (FunctionMap, FunctionGetOutputs, FunctionRetryInputs, AttemptStart, AttemptAwait, AttemptRetry, BlobGet)
- **BlobDownloader** trait for HTTP blob downloads (testable without network)
- **ControlPlaneInvocation**: create via FunctionMap, poll via FunctionGetOutputs, retry via FunctionRetryInputs
- **InputPlaneInvocation**: create via AttemptStart, poll via AttemptAwait, retry via AttemptRetry
- **poll_function_output**: polling loop with 55s default timeout, configurable user timeout
- **process_result**: handles GenericResult status (Success, Timeout, InternalFailure, Failure, Terminated)
- **deserialize_data_format**: CBOR (via ciborium), Pickle (unsupported), ASGI (unsupported), GeneratorDone
- **cbor_serialize/cbor_deserialize**: helpers for function input/output encoding
- **blob_download**: two-step blob retrieval (gRPC for URL, then HTTP download)
- Added `ciborium` dependency for CBOR support
- 48 unit tests covering all operations and edge cases

### Test counts
- Before: 181 unit tests
- After: 229 unit tests (48 new)
- All passing

### What's next (priority order)
1. **Function** (`function.rs`, 10 lines) — Primary user API. Needs FromName, Remote(), Spawn(), GetCurrentStats(), UpdateAutoscaler(), createInput, serialization, invocation routing.
2. **Image.Build()** — Streaming gRPC build orchestration (structural types done, build method not yet implemented).
3. **Queue Put/Get/Iterate** — Need pickle serialization infrastructure.
4. **Integration tests** — All 17 test files in `modal/tests/` are empty.

---

## 2026-03-09 — SandboxFilesystem module implementation

### What was done
Implemented the SandboxFilesystem module (`modal/src/sandbox_filesystem.rs`) with:
- **FileMode** parser/validator for Unix-style file mode strings (r, w, a, x, b, +)
- **FileIO** handle with read/write/seek/flush/close operations, size limit validation, chunked writes
- **SandboxFilesystemService** trait with open, read, readline, write, flush, seek, close, ls, mkdir, rm
- **SandboxFilesystemGrpcClient** abstraction trait for testability
- **SystemErrorCode** mapping from proto errno values to ModalError types
- **FileWatchEvent** and **FileWatchEventType** for filesystem change notifications
- **DirListing** JSON parsing for ls output
- 50 unit tests covering all operations and edge cases

### Test counts
- Before: 131 unit tests
- After: 181 unit tests (50 new)
- All passing

### What's next (priority order)
1. **Invocation** (`invocation.rs`, 6 lines) — Critical foundation for Function execution. Needs control-plane and input-plane implementations, output polling, blob download, result deserialization.
2. **Function** (`function.rs`, 10 lines) — Primary user API. Needs Remote(), Spawn(), GetCurrentStats(), serialization, invocation routing.
3. **Image.Build()** — Streaming gRPC build orchestration (structural types done, build method not yet implemented).
4. **Queue Put/Get/Iterate** — Need pickle serialization infrastructure.
5. **Integration tests** — All 17 test files in `modal/tests/` are empty.

---

## 2026-03-09 — Batch module implementations (Volume, Proxy, FunctionCall, Image, Queue)

### What was done
Implemented 5 modules from stub to working state with full service traits, gRPC client abstraction traits, param structs, and unit tests:

1. **Volume** (`modal/src/volume.rs`): VolumeService (from_name, ephemeral, delete), ephemeral heartbeat, NotFound handling. 11 tests.
2. **Proxy** (`modal/src/proxy.rs`): ProxyService (from_name), NotFound handling. 4 tests.
3. **FunctionCall** (`modal/src/function_call.rs`): FunctionCallService (from_id), Cancel method. 4 tests.
4. **Image** (`modal/src/image.rs`): ImageService (from_registry, from_aws_ecr, from_gcp_artifact_registry, from_id, delete), Layer system, dockerfile_commands chaining, validate_dockerfile_commands. 13 tests.
5. **Queue** (`modal/src/queue.rs`): QueueService (from_name, ephemeral, delete), Queue instance methods (clear, len), partition validation. 15 tests.

### Test counts
- Before: 85 unit tests
- After: 131 unit tests (46 new)
- All passing

### What's next (priority order)
1. **Invocation** (`invocation.rs`, 6 lines) — Critical foundation for Function execution. Needs control-plane and input-plane implementations, output polling, blob download, result deserialization.
2. **Function** (`function.rs`, 10 lines) — Primary user API. Needs Remote(), Spawn(), GetCurrentStats(), serialization, invocation routing.
3. **SandboxFilesystem** (`sandbox_filesystem.rs`, 6 lines) — File I/O via gRPC streaming.
4. **Image.Build()** — Streaming gRPC build orchestration (structural types done, build method not yet implemented).
5. **Queue Put/Get/Iterate** — Need pickle serialization infrastructure.
6. **Integration tests** — All 17 test files in `modal/tests/` are empty.

### Blockers
- `protoc` not pre-installed (needed `apt-get install protobuf-compiler` at start of session).
- Pickle serialization needed for Queue Put/Get and Function invocation — may need external crate.
