fn main() -> Result<(), Box<dyn std::error::Error>> {
    let proto_root = "../libmodal/modal-client";
    let proto_dir = format!("{}/modal_proto", proto_root);

    tonic_build::configure()
        .build_server(false)
        .compile_protos(
            &[
                format!("{}/api.proto", proto_dir),
                format!("{}/task_command_router.proto", proto_dir),
            ],
            &[proto_root],
        )?;

    Ok(())
}
