# Assistant
After the deepseek-ai model is published, The cost of large models will become lower and easier to run locally. The project goal is to deploy multiple large models locally and complete their load balancing.

## Notice
Local model only support chat mode(Deepseek-ai, Qwen). Now
Remote model only support siliconflow model. Now


## Use local model

1. install wasmedge with ggml plugin
``` sh
curl -sSf https://raw.githubusercontent.com/WasmEdge/WasmEdge/master/utils/install.sh | bash -s -- --plugins wasi_nn-ggml
```

2. download ggml model
``` sh
wget https://huggingface.co/Qwen/Qwen1.5-0.5B-Chat-GGUF/resolve/main/qwen1_5-0_5b-chat-q2_k.gguf
```

3. build wasme-ggml
``` sh
cd wasmedge-ggml
rustup target add wasm32-wasi
cargo build --target wasm32-wasi --release
```
4. default config path
``` sh
mkdir -p /etc/assistant/service
cargo run -p assistant-service config > /etc/assistant/service/config.toml
```
## Project Overview

This project implements a C-S AI Service. 
1.support offline and online AI implement. 
2.support multiple clients.
3.support multiple models.
4.support multiple services.

### Support ChatBox Now
you should build with feature `http_api`
and then set the chatbox config like this:
![示例](./images/chatbox.png)


### Local Model Implementation
Use WASM to implement local models, supporting:
In Chat mode:
- Deepseek-ai model
- Qwen model
In Audio mode:
- Chatts (TODO)
- Whisper (TODO)

## Usage

### 1. Service
You can run multiple local models in the service.And online service also support.

The service module handles the core functionality of the voice assistant. To run the service, follow these steps:

#### 1.1. Run

Make sure you have Rust and Cargo installed. Then, navigate to the project root and run:

``` sh
cargo run -p assistant-service
```

#### 1.2. Configure API Key

In the `/etc/assistant/service/config.toml` file, locate the line where the API key is set and replace `<your_api_key>` with your actual API key.

#### 1.3. Choose the model

The Default config give the example.


### 2. Client
Check `src/client/README.md`

## Contributing

Contributions are welcome! Please submit issues or pull requests.

## License

This project is licensed under the MIT License. See the LICENSE file for more details.