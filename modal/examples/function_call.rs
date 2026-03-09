// Rust equivalent of examples/function-call (Go).
//
// Demonstrates calling a Modal Function with positional and keyword arguments.
// Requires a running Modal backend to execute.

use modal::function::{Function, FunctionFromNameParams};

fn main() {
    // Function.from_name resolves a deployed function by app and name.
    let params = FunctionFromNameParams::default();
    println!("Function lookup params - environment: '{}'", params.environment);

    // A Function is created via the FunctionService trait.
    // With a real client:
    //   let echo = function_service.from_name("libmodal-test-support", "echo_string", None)?;
    let echo = Function::new("fn-echo-123".to_string(), None);
    println!("Function ID: {}", echo.function_id);
    println!("Web URL: '{}'", echo.get_web_url());

    // Calling a function remotely:
    //   let result = echo.remote(client, downloader, &args, &kwargs)?;
    // Arguments are CBOR-encoded before sending.
    println!("Function configured for remote invocation.");
}
