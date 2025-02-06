use ttrpc_codegen::{Codegen, Customize, ProtobufCustomize};

const PROTO_FILES: &[&str] = &["./vendor/model.proto"];

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let protobuf_customized = ProtobufCustomize::default().gen_mod_rs(true);
    Codegen::new()
        .out_dir("./src")
        .inputs(PROTO_FILES)
        .include("./vendor")
        .rust_protobuf()
        .customize(Customize {
            async_all: true,
            async_server: false,
            gen_mod: false,
            ..Default::default()
        })
        .rust_protobuf_customize(protobuf_customized.clone())
        .run()
        .expect("Gen code failed.");

    Ok(())
}
