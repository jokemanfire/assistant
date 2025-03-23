use tonic_build;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // tonic_build::configure()
    //     .protoc_arg("--experimental_allow_proto3_optional")
    //     .out_dir("./src/grpc")
    //     .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
    //     .build_server(true)
    //     .build_client(true)
    //     .compile_protos(GRPC_PROTO_FILES, &["./vendor"])
    //     .expect("Failed to generate GRPC bindings");
    tonic_build::configure()
        .protoc_arg("--experimental_allow_proto3_optional")
        .out_dir("./src")
        .type_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .build_server(true)
        .build_client(true)
        .compile(&["proto/service.proto"], &["proto"])?;
    Ok(())
}
