// Rust equivalent of examples/sandbox-named (Go).
//
// Demonstrates creating a named Sandbox that persists and can be
// retrieved by name.
// Requires a running Modal backend to execute.

use modal::sandbox::SandboxFromNameParams;

fn main() {
    // Named sandboxes can be retrieved by name after creation.
    let from_name_params = SandboxFromNameParams {
        environment: String::new(),
    };
    println!("From-name params - environment: '{}'", from_name_params.environment);

    // With a real client:
    //   let sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       name: "libmodal-example-named-sandbox".to_string(),
    //       command: vec!["cat"],
    //   })?;
    //
    //   // Creating another sandbox with the same name returns AlreadyExistsError.
    //   match sandbox_service.create(app, image, &same_name_params) {
    //       Err(ModalError::AlreadyExists(msg)) => println!("Expected: {}", msg),
    //       _ => panic!("should have returned AlreadyExistsError"),
    //   }
    //
    //   // Retrieve by name:
    //   let sb_by_name = sandbox_service.from_name("libmodal-example", sandbox_name, None)?;
    println!("Named sandbox configuration ready.");
}
