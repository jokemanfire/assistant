use anyhow::Result;
use std::collections::HashMap;
use crate::local::manager::ModelRequest;
use wasmedge_sdk::{Module,Vm,Store};
use wasmedge_sdk::plugin::{ExecutionTarget, GraphEncoding, NNPreload, PluginManager};
use wasmedge_sdk::AsInstance;
use wasmedge_sdk::wasi::WasiModule;
use wasmedge_sdk::params;

pub struct WasmModelRunner {
    dir_mapping: HashMap<String, String>,
    wasm_file: String,
    model_name: String,
}

impl WasmModelRunner {
    pub fn new(dir_mapping: HashMap<String, String>, wasm_file: String, model_name: String) -> Result<Self> {
        Ok(Self {
            dir_mapping,
            wasm_file,
            model_name,
        })
    }
    pub async fn deal_request(&self, request: ModelRequest) -> Result<String, Box<dyn std::error::Error>> {
        todo!()
    }

    fn run(&self) -> Result<(), Box<dyn std::error::Error>> {
        // parse arguments
        let args: Vec<String> = std::env::args().collect();
        let dir_mapping = self.dir_mapping.clone();
        let wasm_file = self.wasm_file.clone();
        let model_name = self.model_name.clone();

        // load wasinn-pytorch-plugin from the default plugin directory: /usr/local/lib/wasmedge
        PluginManager::load(None)?;
        // preload named model
        PluginManager::nn_preload(vec![NNPreload::new(
            "default",
            GraphEncoding::GGML,
            ExecutionTarget::AUTO,
            model_name.clone(),
        )]);

        let mut instances = HashMap::new();

        let mut wasi = WasiModule::create(
            Some(vec![wasm_file.as_str(), "default"]),
            Some(vec!["ENCODING=GGML", "TARGET=CPU"]),
            None
        )
        .unwrap();

        instances.insert(wasi.name().to_string(), wasi.as_mut());

        let mut wasi_nn = PluginManager::load_plugin_wasi_nn().unwrap();
        instances.insert(wasi_nn.name().unwrap().to_string(), &mut wasi_nn);

        let store = Store::new(None, instances).unwrap();

        // load wasm module from file
        let module = Module::from_file(None, wasm_file).unwrap();

        // create a Vm
        let mut vm = Vm::new(store);
        vm.register_module(Some("extern"), module).unwrap();

        vm.run_func(Some("extern"), "_start", params!()).unwrap();

        Ok(())
    }
} 

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run() {
        let mut runner = WasmModelRunner::new(HashMap::new(), "/home/10346053@zte.intra/hdy/wasm/wasmedge-ggml-llama.wasm".to_string(), "/home/10346053@zte.intra/hdy/wasm/qwen.gguf".to_string()).unwrap();
        let _ = runner.run().unwrap();
        // println!("result: {}", result);
    }
}