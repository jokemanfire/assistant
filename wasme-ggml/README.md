# compile
cargo build --target wasm32-wasi --release
# run
wasmedge --dir .:./wasme-ggml.wasm