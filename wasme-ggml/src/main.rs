use serde_json::{json, Value};
use std::env;
use std::io;
use std::io::BufRead;
use wasmedge_wasi_nn::{
    self, BackendError, Error, ExecutionTarget, GraphBuilder, GraphEncoding, GraphExecutionContext,
    TensorType,
};
use std::io::Write;
fn read_input() -> String {
    let stdin = io::stdin();
    let mut handle = stdin.lock();
    let mut buf = Vec::new();
    
    handle.read_until(0, &mut buf)
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
        options["ctx-size"] = serde_json::from_str(val.as_str()).expect("invalid ctx-size value (unsigned integer")
    } else {
        options["ctx-size"] = serde_json::from_str("1024").unwrap()
    }
    if let Ok(val) = env::var("stream") {
        options["stream"] = serde_json::from_str(val.as_str()).expect("invalid stream value (true/false)")
    } else {
        options["stream"] = serde_json::from_str("false").unwrap()
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

fn get_data_from_context(context: &GraphExecutionContext, index: usize,is_single: bool) -> String {
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

fn main() {
    let args: Vec<String> = env::args().collect();
    let model_name: &str = &args[1];

    // Set options for the graph. Check our README for more details:
    // https://github.com/second-state/WasmEdge-WASINN-examples/tree/master/wasmedge-ggml#parameters
    let options = get_options_from_env();

    // Create graph and initialize context.
    let graph = GraphBuilder::new(GraphEncoding::Ggml, ExecutionTarget::AUTO)
        .config(serde_json::to_string(&options).expect("Failed to serialize options"))
        .build_from_cache(model_name)
        .expect("Failed to build graph");
    let mut context = graph
        .init_execution_context()
        .expect("Failed to init context");


    loop {
        let input = read_input();
        set_data_to_context(&mut context,  input.as_bytes().to_vec())
            .expect("Failed to set input");


        let stream = options["stream"].as_bool().unwrap_or(false);
        match stream {
            true => {
                loop {
                    match context.compute_single() {
                        Ok(_) => (),
                        Err(Error::BackendError(BackendError::EndOfSequence)) => {
                            break;
                        }
                        Err(Error::BackendError(BackendError::ContextFull)) => {
                            println!("\n[INFO] Context full, we'll reset the context and continue.");
                            break;
                        }
                        Err(Error::BackendError(BackendError::PromptTooLong)) => {
                            println!("\n[INFO] Prompt too long, we'll reset the context and continue.");
                            break;
                        }
                        Err(err) => {
                            println!("\n[ERROR] {}", err);
                            std::process::exit(1);
                        }
                    }
                    // Retrieve the single output token and print it.
                    let token = get_single_output_from_context(&context);
                    std::io::stdout().write_all(token.as_bytes()).expect("Failed to write output");
                    std::io::stdout().flush().expect("Failed to flush stdout");
                }
            }
            false => {
                // Compute the graph.
                match context.compute() {
                    Ok(_) => (),
                    Err(Error::BackendError(BackendError::ContextFull)) => {
                        println!("\n[INFO] Context full, we'll reset the context and continue.");
                    }
                    Err(Error::BackendError(BackendError::PromptTooLong)) => {
                        println!("\n[INFO] Prompt too long, we'll reset the context and continue.");
                    }
                    Err(err) => {
                            println!("\n[ERROR] {}", err);
                    }
                }
                // Retrieve the output.
                let output = get_output_from_context(&context);    

                std::io::stdout().write_all(b"bot:").expect("Failed to write output");
                std::io::stdout().write_all(output.as_bytes()).expect("Failed to write output");
            }
        }
        std::io::stdout().write_all(&[0]).expect("Failed to write null byte");
        std::io::stdout().flush().expect("Failed to flush stdout");

    }
}
