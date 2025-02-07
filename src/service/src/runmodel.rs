use std::io::Error;
use async_trait::async_trait;
use std::process::Command;
use std::time::Duration;

use tokio::io::AsyncWriteExt;

pub struct IoOption{
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub stdin: Option<String>,
}


pub struct ModelRunner{
    pub ioopt: IoOption,
    pub model_path: String,
    pub cli_path: String,
}

impl ModelRunner {
    pub async fn run_model_with_fifo(&self) -> Result<(), Error> {
        let stdout_path = self.ioopt.stdout.clone().unwrap();
        let stdin_path = self.ioopt.stdin.clone().unwrap();
        let stderr_path = self.ioopt.stderr.clone().unwrap();
        let mut sout = sfifo::Sfifo::new(stdout_path)
            .set_create(true)
            .set_write(true)
            .set_blocking(false)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await.unwrap();
        let mut sin = sfifo::Sfifo::new(stdin_path)
            .set_create(true)
            .set_read(true)
            .set_blocking(false)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await.unwrap();
        let mut serr = sfifo::Sfifo::new(stderr_path)
            .set_create(true)
            .set_write(true)
            .set_blocking(false)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await.unwrap();
        // 启动子进程
        println!("Starting child process...");
        let mut child = Command::new(self.cli_path.clone())
            .arg("run")
            .arg("--model")
            .arg(self.model_path.clone())
            .stdin(sin.into_std().await)
            .stdout(sout.into_std().await)
            .stderr(serr.into_std().await)
            .spawn()?;
        
        Ok(())
    }
    pub async fn write_to_model(&self) -> Result<String, Error> {
        let stdout_path = self.ioopt.stdout.clone().unwrap();
        let stdin_path = self.ioopt.stdin.clone().unwrap();
        let stderr_path = self.ioopt.stderr.clone().unwrap();
        // open stdout and stderr read side
        let mut stdout = sfifo::Sfifo::new(stdout_path)
            .set_create(true)
            .set_read(true)
            .set_blocking(false)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await?;
        let mut stderr = sfifo::Sfifo::new(stdin_path)
            .set_create(true)
            .set_read(true)
            .set_blocking(false)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await?;
        // open stdin write side
        let mut fifo = sfifo::Sfifo::new("stdin.fifo")
            .set_create(true)
            .set_write(true)
            .set_blocking(true)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await?;
    
        let input = "your input data here\n";
        fifo.write_all(input.as_bytes()).await.expect("write failed");
        Ok(input.to_string())
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_run_model_with_fifo() {
        let model_runner = ModelRunner{
            ioopt: IoOption{
                stdout: Some("stdout.fifo".to_string()),
                stderr: Some("stderr.fifo".to_string()),
                stdin: Some("stdin.fifo".to_string()),
            },
            model_path: "models/deepseek-ai.DeepSeek-R1-Distill-Qwen-1.5B.Q4_K_M.gguf".to_string(),
            cli_path: "tools/bin/llama-cli".to_string(),

        };
        let _ = model_runner.run_model_with_fifo().await;
        model_runner.write_to_model().await.expect("write to model failed");
        // assert!(true);
    }
}
    



