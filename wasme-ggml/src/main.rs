use serde_json::json;
use serde_json::Value;
use std::env;
use std::io;
use wasmedge_wasi_nn::{
    self, BackendError, Error, ExecutionTarget, GraphBuilder, GraphEncoding, GraphExecutionContext,
    TensorType,
};

fn read_input() -> String {
    loop {
        let mut answer = String::new();
        io::stdin()
            .read_line(&mut answer)
            .expect("Failed to read line");
        if !answer.is_empty() && answer != "\n" && answer != "\r\n" {
            return answer.trim().to_string();
        }
    }
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
    options["ctx-size"] = serde_json::from_str("1024").unwrap();

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

fn get_data_from_context(context: &GraphExecutionContext, index: usize) -> String {
    // Preserve for 4096 tokens with average token length 6
    const MAX_OUTPUT_BUFFER_SIZE: usize = 4096 * 6;
    let mut output_buffer = vec![0u8; MAX_OUTPUT_BUFFER_SIZE];
    let mut output_size = context
        .get_output(index, &mut output_buffer)
        .expect("Failed to get output");
    output_size = std::cmp::min(MAX_OUTPUT_BUFFER_SIZE, output_size);

    return String::from_utf8(output_buffer[..output_size].to_vec())
        .unwrap()
        .to_string();
}

fn get_output_from_context(context: &GraphExecutionContext) -> String {
    get_data_from_context(context, 0)
}

#[allow(dead_code)]
fn get_metadata_from_context(context: &GraphExecutionContext) -> Value {
    serde_json::from_str(&get_data_from_context(context, 1)).expect("Failed to get metadata")
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

        // Retrieve the output.
        let output = get_output_from_context(&context);
        println!("{}\0", output.trim());

    }
}
