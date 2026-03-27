# Modal Rust SDK — Architecture & Feasibility Analysis

## Status: BLOCKED — Modal API is Python-locked

**Date**: 2026-03-12

### The Problem

The Modal `FunctionCreate` gRPC API fundamentally requires a **Python callable**. There is no way to deploy an arbitrary binary as a Modal Function.

The proto definition (`api.proto`) offers two `DefinitionType` values:

1. **`DEFINITION_TYPE_SERIALIZED`** — cloudpickle'd Python bytecode sent as `function_serialized` bytes
2. **`DEFINITION_TYPE_FILE`** — `module_name` + `function_name`, server does `importlib.import_module(module_name).function_name(*args)`

Both paths terminate at Python's `getattr(module, function_name)(*args)`. There is no:
- `entrypoint_args` field (like Sandbox has)
- `DEFINITION_TYPE_CONTAINER` or `DEFINITION_TYPE_BINARY`
- Any language-agnostic function dispatch mechanism

This means the original plan — proc macros (`#[modal::function]`) that compile a Rust binary, upload it to a Modal Image, and register it as a Function — **cannot work** against the current API.

### What Does Work

The **Sandbox API** (`SandboxCreateRequest`) does support `entrypoint_args` and can run arbitrary binaries. The Rust SDK already has full Sandbox support. However, Sandbox lacks:
- Autoscaling
- Web endpoints
- `remote()` / `spawn()` invocation semantics
- Cron/scheduled execution

### Workarounds (all compromised)

1. **Python shim** — A ~20 line Python function in the image that receives Modal inputs, passes them to the Rust binary via stdin/subprocess, and returns stdout. Works but defeats the purpose of a pure Rust SDK.

2. **Sandbox-based functions** — Build a function-like abstraction on top of Sandbox. Create a sandbox per invocation, run the binary, collect output. Loses autoscaling and web endpoints.

3. **Lobby Modal for native binary support** — Ask Modal to add `DEFINITION_TYPE_CONTAINER` or similar. This would allow any language to deploy functions.

### What the Rust SDK Can Still Do

The Rust SDK as a **client SDK** (like the Go SDK) is fully functional:
- Call existing Python-deployed functions via `Function::from_name()` + `remote()`
- Manage all Modal resources (Volumes, Secrets, Images, Queues, Sandboxes, etc.)
- Run arbitrary binaries in Sandboxes
- 466 unit tests + 187 integration tests, all passing

It just can't **deploy** Rust functions as first-class Modal Functions.

### Conclusion

A full Rust deployment SDK (matching Python's `@app.function()`) is not feasible until Modal adds language-agnostic function dispatch to their API. The project is viable as a client SDK only.
