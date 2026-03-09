// Rust equivalent of examples/function-spawn (Go).
//
// Demonstrates spawning a Modal Function asynchronously and retrieving results.
// Requires a running Modal backend to execute.

use modal::function::Function;
use modal::function_call::FunctionCall;

fn main() {
    // Function.spawn starts execution asynchronously, returning a function call ID.
    let function = Function::new("fn-echo-123".to_string(), None);
    println!("Function ID: {}", function.function_id);

    // With a real client:
    //   let echo = function_service.from_name("libmodal-test-support", "echo_string", None)?;
    //   let call_id = echo.spawn(client, &args, &kwargs)?;

    // FunctionCall.from_id wraps a call ID for result retrieval.
    let fc = FunctionCall {
        function_call_id: "fc-call-456".to_string(),
    };
    println!("Function call ID: {}", fc.function_call_id);

    // To get results:
    //   let result = fc.get(ctx, None)?;
    println!("Function spawn configuration ready.");
}
