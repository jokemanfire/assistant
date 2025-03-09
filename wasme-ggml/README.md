# compile
``` sh
rustup target add wasm32-wasi
cargo build --target wasm32-wasi --release
```
# run
``` sh
wasmedge --dir .:./wasme-ggml.wasm
```