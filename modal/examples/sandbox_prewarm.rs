// Rust equivalent of examples/sandbox-prewarm (Go).
//
// Demonstrates pre-building and warming an image using Build() to speed up
// subsequent Sandbox creation.
// Requires a running Modal backend to execute.

use modal::image::{Image, ImageBuildParams, ImageBuildResult, ImageBuildStatus};

fn main() {
    // An image starts without an ID; Build() creates it on Modal.
    let image = Image {
        image_id: String::new(),
        image_registry_config: None,
        tag: "alpine:3.21".to_string(),
        layers: vec![Default::default()],
    };
    println!("Image tag: {}", image.tag);
    println!("Image has ID before build: '{}'", image.image_id);

    // Build parameters reference the App and builder version.
    let build_params = ImageBuildParams {
        app_id: "ap-example".to_string(),
        builder_version: "2024.10".to_string(),
    };
    println!("Build params - app: {}, builder: {}", build_params.app_id, build_params.builder_version);

    // After building, the image has an ID that can be saved for later use.
    let build_result = ImageBuildResult {
        image_id: "im-built-123".to_string(),
        status: ImageBuildStatus::Success,
        exception: None,
    };
    println!("Build status: {:?}, ID: {}", build_result.status, build_result.image_id);

    // Use Image.new() to reference a previously built image by ID.
    let from_id = Image::new(build_result.image_id.clone());
    println!("Image from ID: {}", from_id.image_id);

    // With a real client:
    //   let built = image_service.build(&image, &build_params)?;
    //   let from_id = image_service.from_id(&built.image_id)?;
    //   let sb = sandbox_service.create(app, from_id, &params)?;
    println!("Prewarm configuration ready.");
}
