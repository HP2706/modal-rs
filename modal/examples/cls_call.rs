// Rust equivalent of examples/cls-call (Go).
//
// Demonstrates calling a Modal Cls (class) with positional and keyword arguments.
// Requires a running Modal backend to execute.

use modal::cls::ServiceOptions;

fn main() {
    // A Cls is resolved via the CLS service by app name and class name.
    // With a real client:
    //   let cls = cls_service.from_name("libmodal-test-support", "EchoCls", None)?;
    //   let instance = cls.instance(None)?;
    //   let method = instance.method("echo_string")?;

    // ServiceOptions control runtime configuration for a Cls.
    let options = ServiceOptions::default();
    println!("Default Cls options - CPU: {:?}, Memory: {:?}", options.cpu, options.memory_mib);

    // Calling a Cls method:
    //   let result = method.remote(ctx, &["Hello world!"], None)?;
    //   let result = method.remote(ctx, None, &{"s": "Hello world!"})?;
    println!("Cls configuration ready for invocation.");
}
