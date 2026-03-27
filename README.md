# modal-rs

`modal-rs` is a Rust port of the Go/JS part of Modal's SDK surface, which was built by Claude in a loop, with minimal human intervention.

The agent harness is inspired by anthropics work on getting Claude to autonomously build a C compiler from scratch [Building an efficient C compiler](https://www.anthropic.com/engineering/building-c-compiler). It is essentially just a for loop that spawns a Claude that looks in [PROGRESS.md](./PROGRESS.md), picks up a task, tests and commits it, and updates [PROGRESS.md](./PROGRESS.md) for the next agent.

## Status

The SDK is fully implemented, and all the tests that were in the go/js sdk pass. 
NOTE: I wouldnt recommend using this in production.
For more detail, see [ARCHITECTURE.md](./ARCHITECTURE.md) and [PROGRESS.md](./PROGRESS.md).
