# Modal Rust SDK Progress

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
