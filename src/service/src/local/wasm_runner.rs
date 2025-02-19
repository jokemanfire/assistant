use anyhow::Result;
use std::collections::HashMap;
use std::process::{Child, Command, Stdio};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use tokio::sync::Mutex;
use std::sync::Arc;
use crate::local::manager::ModelRequest;

pub struct WasmModelRunner {
    dir_mapping: HashMap<String, String>,
    wasm_file: String,
    model_name: String,
    process: Option<Arc<Mutex<Child>>>,
}
/// todo use sdk
impl WasmModelRunner {
    pub fn new(dir_mapping: HashMap<String, String>, wasm_file: String, model_name: String) -> Result<Self> {
        Ok(Self {
            dir_mapping,
            wasm_file,
            model_name,
            process: None,
        })
    }

    pub async fn deal_request(&self, request: ModelRequest) -> Result<String, Box<dyn std::error::Error>> {
        if let Some(process) = &self.process {
            let mut process = process.lock().await;
            
            // 获取stdin并写入请求
            let stdin = process.stdin.as_mut().ok_or("Failed to get stdin")?;
            writeln!(stdin, "{}\n", serde_json::to_string(&request)?)?;
            stdin.flush()?;

            // 从stdout读取响应
            let stdout = process.stdout.as_mut().ok_or("Failed to get stdout")?;
            let mut reader = BufReader::new(stdout);
            let mut response = Vec::new();
            reader.read_to_end(&mut response)?;

            Ok(String::from_utf8(response)?)
        } else {
            Err("WASM process not started".into())  
        }
    }

    fn run(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        // 启动 wasmedge 进程
        let process = Command::new("wasmedge")
            .arg("--dir")
            .arg(".")  // 允许访问当前目录
            .arg("--env")
            .arg("llama3=true")
            .arg("--env")
            .arg("n_gpu_layers=100")
            .arg("--nn-preload")
            .arg(format!("default:GGML:AUTO:{}", self.model_name))
            .arg(&self.wasm_file)
            .arg("default")  // 传递 default 作为参数
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        self.process = Some(Arc::new(Mutex::new(process)));
        Ok(())
    }
}

impl Drop for WasmModelRunner {
    fn drop(&mut self) {
        if let Some(process) = &self.process {
            // 尝试获取锁并终止进程
            if let Ok(mut process) = process.try_lock() {
                let _ = process.kill();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_run() {
        let mut runner = WasmModelRunner::new(
            HashMap::new(), 
            "/home/10346053@zte.intra/hdy/github/assistant/target/wasm32-wasi/release/wasme-ggml.wasm".to_string(),
            "/home/10346053@zte.intra/hdy/wasm/qwen.gguf".to_string()
        ).unwrap();
        
        runner.run().unwrap();
        
        let request = ModelRequest {
            prompt: "测试输入".to_string(),
            parameters: None,
            request_id: "test-1".to_string(),
        };
        let response = runner.deal_request(request).await.unwrap();
        println!("Response: {}", response);
    }
}