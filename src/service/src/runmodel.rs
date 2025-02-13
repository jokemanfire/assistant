use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::Duration;
use std::{io::Error, process::Child};
use tokio::sync::Mutex;

use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct IoOption {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub stdin: Option<String>,
}

pub struct ModelRunner {
    pub ioopt: IoOption,
    pub model_path: String,
    pub cli_path: String,
    pub request_receiver: Arc<Mutex<tokio::sync::mpsc::Receiver<String>>>,
    pub result_sender: Arc<tokio::sync::mpsc::Sender<String>>,
    pub child: Option<Child>,
}

impl ModelRunner {
    pub async fn handle_requests_and_read_output(&mut self) -> Result<(), Error> {
        let stdout_path = self.ioopt.stdout.clone().unwrap();
        let stdin_path = self.ioopt.stdin.clone().unwrap();
        let stderr_path = self.ioopt.stderr.clone().unwrap();

        let (s, r) = tokio::sync::mpsc::channel::<String>(1);
        let request_receiver = self.request_receiver.clone();
        tokio::task::spawn(async move {
            let mut stdin = sfifo::Sfifo::new(stdin_path)
                .set_write(true)
                .set_blocking(true)
                .set_timeout(Duration::from_secs(5))
                .open()
                .await
                .unwrap();

            while let Some(mut request) = request_receiver.lock().await.recv().await {
                // add \n here
                request.push('\n');
                println!("Received stdin request: {}", request);
                let _ = stdin.write_all(request.as_bytes()).await.unwrap();
                println!("Write stdin complete");
            }
        });

        let result_sender = self.result_sender.clone();
        tokio::task::spawn(async move {
            let mut stdout = sfifo::Sfifo::new(stdout_path)
                .set_read(true)
                .set_blocking(true)
                .set_timeout(Duration::from_secs(5))
                .open()
                .await
                .unwrap();
            let mut tmp_buf = Vec::with_capacity(1024);
            while let Ok(_) = stdout.read_buf(&mut tmp_buf).await {
                println!("Received stdout result: {:?}", &tmp_buf);
                let r = String::from_utf8_lossy(&tmp_buf);

                let _ = result_sender.send(r.to_string()).await;
                tmp_buf.clear();
            }
        });
        let result_sender2 = self.result_sender.clone();
        tokio::task::spawn(async move {
            let mut serr = sfifo::Sfifo::new(stderr_path)
                .set_read(true)
                .set_blocking(true)
                .set_timeout(Duration::from_secs(5))
                .open()
                .await
                .unwrap();
            let mut tmp_buf = Vec::with_capacity(1024);
            while let Ok(_) = serr.read_buf(&mut tmp_buf).await {
                let r = String::from_utf8_lossy(&tmp_buf);
                // println!("Received stderror result: {}", r);
                // let _ = result_sender2.send(r.to_string()).await;
                tmp_buf.clear();
            }
        });
        Ok(())
    }
    pub async fn run_model_with_fifo(&mut self) -> Result<(), Error> {
        let stdout_path = self.ioopt.stdout.clone().unwrap();
        let stdin_path = self.ioopt.stdin.clone().unwrap();
        let stderr_path = self.ioopt.stderr.clone().unwrap();
        // copy stdout and stderr
        self.handle_requests_and_read_output().await?;
        let sout = sfifo::Sfifo::new(stdout_path)
            .set_create(true)
            .set_write(true)
            .set_blocking(true)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await
            .unwrap();
        let sin = sfifo::Sfifo::new(stdin_path)
            .set_create(true)
            .set_read(true)
            .set_blocking(true)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await
            .unwrap();
        let serr = sfifo::Sfifo::new(stderr_path)
            .set_create(true)
            .set_write(true)
            .set_blocking(true)
            .set_timeout(Duration::from_secs(5))
            .open()
            .await
            .unwrap();

        println!("Starting child process...");
        let child = Command::new(self.cli_path.clone())
            .arg("--model")
            .arg(self.model_path.clone())
            .stdin(sin.into_std().await)
            .stdout(sout.into_std().await)
            .stderr(serr.into_std().await)
            .env("LD_LIBRARY_PATH", "/usr/local/bin/llama")
            .spawn()
            .unwrap();
        // let mut child = Command::new(self.cli_path.clone())
        // // .arg("run")
        // .arg("--model")
        // .arg(self.model_path.clone())
        // .stdin(Stdio::null())
        // .stdout(Stdio::piped())
        // .stderr(Stdio::piped())
        // .spawn().unwrap();
        // let r = child.wait_with_output().unwrap();
        // println!("result {:?}", r);
        // tokio::time::sleep(Duration::from_secs(5)).await;
        println!("llama cli started");
        self.child = Some(child);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio;

    #[tokio::test]
    async fn test_run_model_with_fifo() {}
}
