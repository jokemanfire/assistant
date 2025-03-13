use serde_json::{json, Value};
use std::env;
use std::io;
use std::io::BufRead;
use std::io::Write;
use wasmedge_wasi_nn::{
    self, BackendError, Error, ExecutionTarget, GraphBuilder, GraphEncoding, GraphExecutionContext,
    TensorType,
};

fn read_input() -> String {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buf = Vec::new();

    handle
        .read_until(0, &mut buf)
        .expect("Failed to read input");

    // Remove the null terminator if present
    if buf.last() == Some(&0) {
        buf.pop();
    }

    String::from_utf8(buf)
        .expect("Invalid UTF-8")
        .trim()
        .to_string()
}

fn get_options_from_env() -> Value {
    let mut options = json!({});
    if let Ok(val) = env::var("enable_log") {
        options["enable-log"] = serde_json::from_str(val.as_str())
            .expect("invalid value for enable-log option (true/false)")
    } else {
        options["enable-log"] = serde_json::from_str("false").unwrap()
    }
    if let Ok(val) = env::var("n_gpu_layers") {
        options["n-gpu-layers"] =
            serde_json::from_str(val.as_str()).expect("invalid ngl value (unsigned integer")
    } else {
        options["n-gpu-layers"] = serde_json::from_str("0").unwrap()
    }
    if let Ok(val) = env::var("ctx-size") {
        options["ctx-size"] =
            serde_json::from_str(val.as_str()).expect("invalid ctx-size value (unsigned integer")
    } else {
        options["ctx-size"] = serde_json::from_str("1024").unwrap()
    }
    if let Ok(val) = env::var("stream") {
        options["stream"] =
            serde_json::from_str(val.as_str()).expect("invalid stream value (true/false)")
    } else {
        options["stream"] = serde_json::from_str("false").unwrap()
    }
    if let Ok(val) = env::var("model_type") {
        options["model-type"] = json!(val);
    }

    options
}

fn set_data_to_context(context: &mut GraphExecutionContext, data: Vec<u8>) -> Result<(), Error> {
    context.set_input(0, TensorType::U8, &[1], &data)
}

#[allow(dead_code)]
fn set_metadata_to_context(
    context: &mut GraphExecutionContext,
    data: Vec<u8>,
) -> Result<(), Error> {
    context.set_input(1, TensorType::U8, &[1], &data)
}

fn get_data_from_context(context: &GraphExecutionContext, index: usize, is_single: bool) -> String {
    // Preserve for 4096 tokens with average token length 6
    const MAX_OUTPUT_BUFFER_SIZE: usize = 4096 * 6;
    let mut output_buffer = vec![0u8; MAX_OUTPUT_BUFFER_SIZE];
    let mut output_size = if is_single {
        context
            .get_output_single(index, &mut output_buffer)
            .expect("Failed to get single output")
    } else {
        context
            .get_output(index, &mut output_buffer)
            .expect("Failed to get output")
    };
    output_size = std::cmp::min(MAX_OUTPUT_BUFFER_SIZE, output_size);

    return String::from_utf8(output_buffer[..output_size].to_vec())
        .unwrap()
        .to_string();
}

fn get_output_from_context(context: &GraphExecutionContext) -> String {
    get_data_from_context(context, 0, false)
}

fn get_single_output_from_context(context: &GraphExecutionContext) -> String {
    get_data_from_context(context, 0, true)
}

#[allow(dead_code)]
fn get_metadata_from_context(context: &GraphExecutionContext) -> Value {
    serde_json::from_str(&get_data_from_context(context, 1, false)).expect("Failed to get metadata")
}

// output error message
fn output_error(error_msg: &str) {
    let output = format!("bot:ERROR: {}", error_msg);
    std::io::stdout()
        .write_all(output.as_bytes())
        .expect("Failed to write output");
    std::io::stdout()
        .write_all(&[0])
        .expect("Failed to write null byte");
    std::io::stdout().flush().expect("Failed to flush stdout");
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        output_error("Missing model name argument");
        std::process::exit(1);
    }

    let model_name: &str = &args[1];

    // record model loading start
    eprintln!("Loading model: {}", model_name);

    // Set options for the graph
    let options = get_options_from_env();

    // record config info
    eprintln!(
        "Options: {}",
        serde_json::to_string(&options).unwrap_or_default()
    );

    // Create graph and initialize context
    let graph = match GraphBuilder::new(GraphEncoding::Ggml, ExecutionTarget::AUTO)
        .config(serde_json::to_string(&options).expect("Failed to serialize options"))
        .build_from_cache(model_name)
    {
        Ok(g) => g,
        Err(e) => {
            let error_msg = format!("Failed to build graph: {}", e);
            eprintln!("{}", error_msg);
            output_error(&error_msg);
            std::process::exit(1);
        }
    };

    let mut context = match graph.init_execution_context() {
        Ok(ctx) => ctx,
        Err(e) => {
            let error_msg = format!("Failed to init context: {}", e);
            eprintln!("{}", error_msg);
            output_error(&error_msg);
            std::process::exit(1);
        }
    };

    // record model loaded successfully
    eprintln!("Model loaded successfully");

    loop {
        let input = read_input();

        // record received input length
        eprintln!("Received input of length: {}", input.len());

        if input.is_empty() {
            output_error("Empty input received");
            continue;
        }

        match set_data_to_context(&mut context, input.as_bytes().to_vec()) {
            Ok(_) => (),
            Err(e) => {
                let error_msg = format!("Failed to set input: {}", e);
                eprintln!("{}", error_msg);
                output_error(&error_msg);
                continue;
            }
        }

        let stream = options["stream"].as_bool().unwrap_or(false);
        match stream {
            true => {
                let mut has_output = false;
                let mut error_occurred = false;
                let mut error_message = String::new();

                loop {
                    match context.compute_single() {
                        Ok(_) => (),
                        Err(Error::BackendError(BackendError::EndOfSequence)) => {
                            break;
                        }
                        Err(Error::BackendError(BackendError::ContextFull)) => {
                            error_message = "Context full, please reduce input length".to_string();
                            error_occurred = true;
                            break;
                        }
                        Err(Error::BackendError(BackendError::PromptTooLong)) => {
                            error_message =
                                "Prompt too long, please reduce input length".to_string();
                            error_occurred = true;
                            break;
                        }
                        Err(err) => {
                            error_message = format!("Error: {}", err);
                            error_occurred = true;
                            break;
                        }
                    }

                    // Retrieve the single output token and print it
                    let token = get_single_output_from_context(&context);
                    if !token.is_empty() {
                        has_output = true;
                        std::io::stdout()
                            .write_all(token.as_bytes())
                            .expect("Failed to write output");
                        std::io::stdout().flush().expect("Failed to flush stdout");
                    }
                }
                if error_occurred {
                    eprintln!("{}", error_message);
                    if !has_output {
                        // if no output, output error message
                        output_error(&error_message);
                        continue;
                    }
                }
            }
            false => {
                // Compute the graph
                let result = context.compute();
                match result {
                    Ok(_) => {
                        // Retrieve the output
                        let output = get_output_from_context(&context);

                        if output.trim().is_empty() {
                            eprintln!("Model returned empty output");
                            output_error("Model returned empty output, try with different prompt");
                            continue;
                        }

                        std::io::stdout()
                            .write_all(b"bot:")
                            .expect("Failed to write output");
                        std::io::stdout()
                            .write_all(output.as_bytes())
                            .expect("Failed to write output");
                    }
                    Err(Error::BackendError(BackendError::ContextFull)) => {
                        let error_msg = "Context full, please reduce input length";
                        eprintln!("{}", error_msg);
                        output_error(error_msg);
                        continue;
                    }
                    Err(Error::BackendError(BackendError::PromptTooLong)) => {
                        let error_msg = "Prompt too long, please reduce input length";
                        eprintln!("{}", error_msg);
                        output_error(error_msg);
                        continue;
                    }
                    Err(err) => {
                        let error_msg = format!("Error: {}", err);
                        eprintln!("{}", error_msg);
                        output_error(&error_msg);
                        continue;
                    }
                }
            }
        }

        std::io::stdout()
            .write_all(&[0])
            .expect("Failed to write null byte");
        std::io::stdout().flush().expect("Failed to flush stdout");
    }
}
