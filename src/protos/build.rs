use tonic_build;

const GRPC_PROTO_FILES: &[&str] = &["./vendor/mserver.proto", "./vendor/model.proto"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .out_dir("./src/grpc")
        // .compile_well_known_types(true)
        .build_server(true)
        .build_client(true)
        .compile_protos(GRPC_PROTO_FILES, &["./vendor"])
        .expect("Failed to generate GRPC bindings");
    Ok(())
}
