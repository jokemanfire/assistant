# Assistant Client

This is a command line client for interacting with the Assistant service. It uses the gRPC protocol to communicate with the server, providing functions for querying models, checking model status, sending messages, and conducting interactive chat sessions.

## Features

- List available models
- Check the status of a specific model
- Send a single message and get a response
- Conduct interactive chat sessions

## Usage

### Compile

```bash
cargo build --release
```

### Run

```bash
# Default connect to http://127.0.0.1:50051
./target/release/assistant-client [command]

# Specify server address
./target/release/assistant-client -e http://your-server:port [command]
```

### Commands

#### List available models

```bash
./target/release/assistant-client list-models
```

#### Check model status

```bash
./target/release/assistant-client status <model_id>
```

#### Send a single message

```bash
./target/release/assistant-client message "Your message content"

# With system prompt
./target/release/assistant-client message -s "You are a helpful assistant" "Your message content"
```

#### Interactive chat

```bash
./target/release/assistant-client chat

# With system prompt
./target/release/assistant-client chat -s "You are a helpful assistant"
```

## Example

```bash
# List all available models
./target/release/assistant-client list-models

# Check the status of a specific model
./target/release/assistant-client status "Qwen/Qwen2.5-7B-Instruct"

# Send a single message
./target/release/assistant-client message "What's the weather in Beijing today?"

# Start interactive chat
./target/release/assistant-client chat
``` 