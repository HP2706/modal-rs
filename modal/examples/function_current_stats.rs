// Rust equivalent of examples/function-current-stats (Go).
//
// Demonstrates retrieving current statistics for a Modal Function.
// Requires a running Modal backend to execute.

use modal::function::{Function, FunctionStats};

fn main() {
    let function = Function::new("fn-echo-123".to_string(), None);
    println!("Function ID: {}", function.function_id);

    // With a real client:
    //   let stats = function.get_current_stats(grpc_client)?;
    // FunctionStats contains backlog and runner information.
    let stats = FunctionStats {
        backlog: 0,
        num_total_runners: 0,
    };
    println!("Function Statistics:");
    println!("  Backlog: {} inputs", stats.backlog);
    println!("  Total Runners: {} containers", stats.num_total_runners);
}
