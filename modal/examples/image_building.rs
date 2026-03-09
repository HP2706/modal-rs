// Rust equivalent of examples/image-building (Go).
//
// Demonstrates building and caching a Docker image with custom system
// dependencies using Image.dockerfile_commands() and layer chaining.
// Requires a running Modal backend to execute.

use modal::image::{Image, ImageDockerfileCommandsParams};
use modal::secret::Secret;

fn main() {
    // Create an ephemeral secret for build-time environment variables.
    let secret = Secret {
        secret_id: "st-curl-version".to_string(),
        name: String::new(),
    };

    // Build an image with chained Dockerfile commands.
    // Each dockerfile_commands() call creates a new layer.
    let base = Image {
        image_id: String::new(),
        image_registry_config: None,
        tag: "alpine:3.21".to_string(),
        layers: vec![Default::default()],
    };

    let image = base
        .dockerfile_commands(
            &["RUN apk add --no-cache curl=$CURL_VERSION".to_string()],
            Some(&ImageDockerfileCommandsParams {
                secrets: vec![secret],
                ..Default::default()
            }),
        )
        .dockerfile_commands(
            &["ENV SERVER=ipconfig.me".to_string()],
            None,
        );

    println!("Image tag: {}", image.tag);
    println!("Image layers: {}", image.layers.len());
    for (i, layer) in image.layers.iter().enumerate() {
        println!("  Layer {}: {:?}", i, layer.commands);
    }

    // With a real client:
    //   let built = image_service.build(&image, &build_params)?;
    //   let sb = sandbox_service.create(app, built, &params)?;
    println!("Image with {} layers ready for build.", image.layers.len());
}
