use ttrpc_codegen::{Codegen, Customize, ProtobufCustomize};

const PROTO_FILES: &[&str] = &["./vendor/model.proto"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protobuf_customized = ProtobufCustomize::default();
    Codegen::new()
        .out_dir("./src")
        .inputs(PROTO_FILES)
        .include("./vendor")
        .rust_protobuf()
        .customize(Customize {
            async_all: true,
            gen_mod: false,
            ..Default::default()
        })
        .rust_protobuf_customize(protobuf_customized.clone())
        .run()
        .expect("Gen code failed.");

    tonic_build::configure()
        .out_dir("./src/grpc")
        .compile_well_known_types(true)
        .build_server(true)
        .build_client(true)
        .compile_protos(PROTO_FILES, &["./vendor"])
        .expect("Failed to generate GRPC bindings");
    Ok(())
}
