use ttrpc_codegen::{Codegen, Customize, ProtobufCustomize};

const PROTO_FILES: &[&str] = &[
    "src/vendor/api.proto",
];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = std::fs::create_dir("src/protocols");
    let protobuf_customized = ProtobufCustomize::default().gen_mod_rs(true);

    Codegen::new()
        .out_dir("src/protocols")
        .inputs(PROTO_FILES)
        .include("src/vendor")
        .rust_protobuf()
        .customize(Customize {
            async_all: true,
            async_server: false,
            gen_mod: true,
            ..Default::default()
        })
        .rust_protobuf_customize(protobuf_customized.clone())
        .run()
        .expect("Gen sync code failed.");

    Ok(())
}
