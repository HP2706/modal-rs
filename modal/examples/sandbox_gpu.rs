// Rust equivalent of examples/sandbox-gpu (Go).
//
// Demonstrates creating a Sandbox with GPU access.
// Requires a running Modal backend to execute.

use modal::app::parse_gpu_config;
use modal::image::Image;

fn main() {
    // Parse GPU configuration string.
    let gpu_config = parse_gpu_config("A10G").unwrap();
    println!("GPU type: {}, count: {}", gpu_config.gpu_type, gpu_config.count);

    // Multi-GPU configuration:
    let multi_gpu = parse_gpu_config("A100:2").unwrap();
    println!("GPU type: {}, count: {}", multi_gpu.gpu_type, multi_gpu.count);

    // NVIDIA CUDA image for GPU workloads.
    let image = Image {
        image_id: String::new(),
        image_registry_config: None,
        tag: "nvidia/cuda:12.4.0-devel-ubuntu22.04".to_string(),
        layers: vec![Default::default()],
    };
    println!("Image tag: {}", image.tag);

    // With a real client:
    //   let sb = sandbox_service.create(app, image, &SandboxCreateParams {
    //       gpu: "A10G",
    //       ..Default::default()
    //   })?;
    //   let p = sb.exec(["nvidia-smi"], None)?;
    //   let output = p.stdout.read_to_string()?;
    println!("GPU sandbox configuration ready.");
}
