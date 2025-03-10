# Assistant
After the deepseek-ai model is published, The cost of large models will become lower and easier to run locally. The project goal is to deploy multiple large models locally and complete their load balancing.


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
cargo run --bin service config > /etc/assistant/service/config.toml
```
## Project Overview

This project implements a C-S AI assistant. 
1.support offline and online AI implement. 
2.support multiple clients.
3.support multiple models.
4.support multiple services.



## Design
### System Architecture
The system follows a modular client-server architecture with the following key components:

1. **Service Interface Layer**
   - Supports multiple protocols (TTRPC, gRPC)
   - Handles client connections and request routing
   - Provides unified API interface for different clients

2. **Load Balancer**
   - Distributes requests across model instances
   - Manages resource allocation
   - Ensures high availability and fault tolerance

3. **Model Management System**
   - **Model Registry**: Manages model registration and lifecycle
   - **Local Models**: Supports offline model deployment using WASM
   - **Remote Services**: Integrates with cloud-based AI services

4. **Processing Pipeline**
   - Speech-to-Text (STT) processing
   - Dialogue management
   - Text-to-Speech (TTS) conversion



### Local Model Implementation
Use WASM to implement local models, supporting:
In Chat mode:
- Deepseek-ai model
- Qwen model
In Audio mode:
- Chatts
- Whisper

## Usage

### 1. Service
You can run multiple local models in the service.And online service also support.

The service module handles the core functionality of the voice assistant. To run the service, follow these steps:

#### 1.1. Install Dependencies

Make sure you have Rust and Cargo installed. Then, navigate to the project root and run:

``` sh
cargo run --bin service
```

#### 1.2. Configure API Key

In the `src/service/src/default.toml` file, locate the line where the API key is set and replace `<your_api_key>` with your actual API key.

#### 1.3. Run the Service

To start the service, run the following command from the project root:
#### 1.4. Choose the model

You can choose the model in the `src/service/src/default.toml` file.

``` toml
[dialogue_model]
model_path="https://api.siliconflow.cn/v1/chat/completions"
api_key= ""
model_name= "Qwen/Qwen2.5-7B-Instruct"
stream = true
prompt_path= ""
```

### 2. Client

The client module provides multiple binaries for different use cases. Below are instructions for each client type.

#### 2.1. Web Client

The web client provides a simple HTML interface for user interaction.

##### 2.1.1. Run the Web Client

To start the web client, run:
``` sh
cargo run --bin web
``` 
You can then access the web interface at `http://127.0.0.1:3030/config`.

example:
![index](images/index.png)
![chat](images/chat.png)

#### 2.2. CLI Client

The command-line interface client allows you to interact with the service via the terminal.

##### 2.2.1. Run the CLI Client

To start the CLI client, run:
``` sh
cargo run --bin cli
```
Follow the prompts to input your audio data and receive responses.

#### 2.3. GUI Client (if applicable)

If you have a GUI client implemented, you can run it similarly:
``` sh
cargo run --bin gui
``` 
![gui](images/UIChat.png)

### 3. Protos

The `protos` directory contains Protocol Buffers definitions used for communication between the service and clients. Ensure that you compile the proto files if you make any changes.

## Notes

- Ensure that your system has a working microphone. If the microphone is not detected or not functioning properly, the voice-to-text module will not be able to capture audio data.
- Adjust the audio data processing logic as needed, especially for buffering and chunking. Different microphones and audio devices may have varying sample rates and formats, so you may need to adjust the code accordingly.
- The audio playback implementation may need to be adjusted based on the audio playback library you choose. The current project uses `rodio` for audio playback, but you can select other libraries as needed.

## Contributing

Contributions are welcome! Please submit issues or pull requests.

## License

This project is licensed under the MIT License. See the LICENSE file for more details.