use anyhow::Result;
use model::LLMModel;
use std::{io::Write, path::PathBuf};

pub mod model;
pub mod sampler;
mod token_output_stream;

pub async fn run_model(gguf_path: PathBuf) -> Result<()> {
    let mut llm = LLMModel::new(gguf_path)?;

    // Encode the prompt.
    println!("Encoding & loading prompt...");
    let prompt = r#"
        <|begin_of_text|><|start_header_id|>system<|end_header_id|>

        You are a helpful AI assistant.<|eot_id|>
        <|start_header_id|>user<|end_header_id|>

        What is the airspeed velocity of an unladen swallow?<|eot_id|>
        <|start_header_id|>assistant<|end_header_id|>
    "#;
    let mut all_tokens = vec![];
    let mut sampler = llm.sampler();

    // process prompt
    let mut timer = std::time::Instant::now();
    let next_token = sampler.load_prompt(&prompt)?;
    log::info!("processing prompt in {:.3}s", timer.elapsed().as_secs_f32());

    all_tokens.push(next_token);
    if let Some(t) = llm.stream.next_token(next_token)? {
        print!("{t}");
        std::io::stdout().flush()?;
    }

    timer = std::time::Instant::now();
    let mut sampled = 1;
    let num_tokens_to_sample = 1024;

    for _ in 0..num_tokens_to_sample {
        let next_token = sampler.next()?;
        all_tokens.push(next_token);
        if let Some(t) = llm.stream.next_token(next_token)? {
            print!("{t}");
            std::io::stdout().flush()?;
        }

        sampled += 1;
        if sampler.is_done() {
            println!("\n--------------------------------------------------");
            println!("Got EOS after {sampled} tokens");
            break;
        };
    }

    if let Some(rest) = llm.stream.decode_rest().map_err(candle::Error::msg)? {
        print!("{rest}");
    }
    std::io::stdout().flush()?;
    log::info!(
        "{sampled:4} tokens generated: {:.2} token/s",
        sampled as f64 / timer.elapsed().as_secs_f64(),
    );

    Ok(())
}
