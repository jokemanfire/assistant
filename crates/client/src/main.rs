use anyhow::Result;
use assistant_client::AssistantClient;
use clap::{Parser, Subcommand};
use log::{error, info};
use protos::grpc::model::ChatMessage;
use std::io::{self, BufRead, Write};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// gRPC server endpoint
    #[arg(short, long, default_value = "http://127.0.0.1:50051")]
    endpoint: String,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List available models
    ListModels,

    /// Check status of a specific model
    Status {
        /// Model ID to check
        #[arg(required = true)]
        model_id: String,
    },

    /// Start an interactive chat session
    Chat {
        /// Optional system prompt
        #[arg(short, long)]
        system_prompt: Option<String>,
    },

    /// Send a single message and get a response
    Message {
        /// Message content
        #[arg(required = true)]
        content: String,

        /// Optional system prompt
        #[arg(short, long)]
        system_prompt: Option<String>,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    // Parse command line arguments
    let cli = Cli::parse();

    // Connect to the server
    let mut client = AssistantClient::connect(&cli.endpoint).await?;

    // Process commands
    match &cli.command {
        Commands::ListModels => {
            info!("Querying available models...");
            let models = client.query_models().await?;

            println!("Available models:");
            for model in models.models {
                let status = match model.status {
                    0 => "Unknown",
                    1 => "Loading",
                    2 => "Ready",
                    3 => "Error",
                    _ => "Invalid status",
                };

                println!("- ID: {}", model.model_id);
                println!("  Name: {}", model.name);
                println!("  Description: {}", model.description);
                println!("  Status: {}", status);
                println!();
            }
        }

        Commands::Status { model_id } => {
            info!("Checking status of model: {}", model_id);
            let status = client.query_model_status(model_id).await?;
            println!("Model status: {}", status);
        }

        Commands::Chat { system_prompt } => {
            info!("Starting interactive chat session...");

            // Create initial messages with system prompt if provided
            let mut messages: Vec<ChatMessage> = Vec::new();

            if let Some(prompt) = system_prompt {
                messages.push(AssistantClient::create_system_message(prompt));
                println!("System prompt set.");
            }

            println!("Enter your messages (type 'exit' to quit):");

            let stdin = io::stdin();
            let mut stdout = io::stdout();

            loop {
                print!("> ");
                stdout.flush()?;

                let mut input = String::new();
                stdin.lock().read_line(&mut input)?;

                let input = input.trim();
                if input.eq_ignore_ascii_case("exit") {
                    break;
                }

                // Add user message
                messages.push(AssistantClient::create_user_message(input));

                // Get response
                match client.chat_stream(messages.clone()).await {
                    Ok(mut stream) => {
                        print!("Assistant: ");
                        stdout.flush()?;

                        let mut full_response = String::new();

                        // receive stream response
                        while let Some(chunk) = stream.recv().await {
                            // check if error
                            if chunk.starts_with("Error:") || chunk.starts_with("Failed to connect")
                            {
                                error!("{}", chunk);
                                println!("\nError: Failed to get streaming response.");
                                break;
                            }
                            // print chunk
                            print!("{}", chunk);
                            stdout.flush()?;

                            // accumulate full response
                            full_response.push_str(&chunk);
                        }

                        println!();

                        // add assistant response to message history
                        messages.push(AssistantClient::create_assistant_message(&full_response));
                    }
                    Err(e) => {
                        error!("Error: {}", e);
                        println!("Error: Failed to get response from the assistant.");
                    }
                }
            }
        }

        Commands::Message {
            content,
            system_prompt,
        } => {
            // Create messages
            let mut messages: Vec<ChatMessage> = Vec::new();

            if let Some(prompt) = system_prompt {
                messages.push(AssistantClient::create_system_message(prompt));
            }

            messages.push(AssistantClient::create_user_message(content));

            // Get response
            let response = client.chat(messages).await?;
            println!("{}", response);
        }
    }

    Ok(())
}
