use anyhow::Result;
use candle::{quantized::gguf_file, Tensor};
use candle_transformers::{
    generation::{LogitsProcessor, Sampling},
    models::quantized_llama::ModelWeights,
};
use std::{env::current_dir, io::Write};
use tokenizers::Tokenizer;

mod token_output_stream;
use token_output_stream::TokenOutputStream;

fn format_size(size_in_bytes: usize) -> String {
    if size_in_bytes < 1_000 {
        format!("{}B", size_in_bytes)
    } else if size_in_bytes < 1_000_000 {
        format!("{:.2}KB", size_in_bytes as f64 / 1e3)
    } else if size_in_bytes < 1_000_000_000 {
        format!("{:.2}MB", size_in_bytes as f64 / 1e6)
    } else {
        format!("{:.2}GB", size_in_bytes as f64 / 1e9)
    }
}

pub async fn run_model() -> Result<String> {
    let start = std::time::Instant::now();

    let _ = dbg!(current_dir());
    let mut file =
        std::fs::File::open("../../assets/models/llm/Llama-3.2-3B-Instruct.Q5_K_M.gguf")?;
    let device = candle::Device::Cpu;

    let model = gguf_file::Content::read(&mut file)?;
    let mut total_size_in_bytes = 0;
    for (_, tensor) in model.tensor_infos.iter() {
        let elem_count = tensor.shape.elem_count();
        total_size_in_bytes +=
            elem_count * tensor.ggml_dtype.type_size() / tensor.ggml_dtype.block_size();
    }
    println!(
        "loaded {:?} tensors ({}) in {:.2}s",
        model.tensor_infos.len(),
        &format_size(total_size_in_bytes),
        start.elapsed().as_secs_f32(),
    );

    // let tokenizer = Tokenizer::from_pretrained("llama-3");
    dbg!(model.metadata.keys());
    // todo: load tokenizer from gguf file itself.
    println!("Loading tokenizer & weights");
    let model_load = std::time::Instant::now();
    let tokenizer = Tokenizer::from_file("../../assets/models/llm/llama-3-tokenizer.json")
        .expect("Unable to open tokenizers file");
    let mut weights = ModelWeights::from_gguf(model, &mut file, &device)?;
    let mut tos = TokenOutputStream::new(tokenizer);
    println!(
        "Model load took: {:.3}s",
        model_load.elapsed().as_secs_f32()
    );

    // Encode the prompt.
    println!("Encoding & loading prompt...");
    let prompt = "What is the airspeed velocity of an unladen swallow?";
    let tokens = tos
        .tokenizer()
        .encode(prompt, true)
        .map_err(anyhow::Error::msg)?;

    let prompt_tokens = [tokens.get_ids()].concat();

    let mut all_tokens = vec![];
    let mut logits_processor = {
        let sampling = Sampling::ArgMax;
        LogitsProcessor::from_sampling(0, sampling)
    };

    let mut next_token = 0;

    // process prompt
    let start_prompt_processing = std::time::Instant::now();
    let input = Tensor::new(prompt_tokens.as_slice(), &device)?.unsqueeze(0)?;
    let logits = weights.forward(&input, 0)?;
    let logits = logits.squeeze(0)?;
    logits_processor.sample(&logits)?;
    let prompt_dt = start_prompt_processing.elapsed();
    println!("processed prompt in {:.3}s", prompt_dt.as_secs_f32());
    all_tokens.push(next_token);
    if let Some(t) = tos.next_token(next_token)? {
        print!("{t}");
        std::io::stdout().flush()?;
    }

    let eos_token = "<|end_of_text|>";
    let eos_token = *tos.tokenizer().get_vocab(true).get(eos_token).unwrap();

    let mut sampled = 1;
    let num_tokens_to_sample = 1024;
    let start_post_prompt = std::time::Instant::now();
    for index in 0..num_tokens_to_sample {
        let input = Tensor::new(&[next_token], &device)?.unsqueeze(0)?;
        let logits = weights.forward(&input, prompt_tokens.len() + index)?;
        let logits = logits.squeeze(0)?;
        next_token = logits_processor.sample(&logits)?;
        all_tokens.push(next_token);
        if let Some(t) = tos.next_token(next_token)? {
            print!("{t}");
            std::io::stdout().flush()?;
        }
        sampled += 1;
        if next_token == eos_token {
            println!("Got EOS after {sampled} tokens");
            break;
        };
    }
    if let Some(rest) = tos.decode_rest().map_err(candle::Error::msg)? {
        print!("{rest}");
    }
    std::io::stdout().flush()?;
    let dt = start_post_prompt.elapsed();
    println!(
        "\n\n{:4} prompt tokens processed: {:.2} token/s",
        prompt_tokens.len(),
        prompt_tokens.len() as f64 / prompt_dt.as_secs_f64(),
    );
    println!(
        "{sampled:4} tokens generated: {:.2} token/s",
        sampled as f64 / dt.as_secs_f64(),
    );

    Ok("ADLAJF".into())
}

#[cfg(test)]

mod tests {
    use super::*;

    #[tokio::test]
    async fn it_works() {
        let result = run_model().await.unwrap();
        assert_eq!(result, "adlfkjd");
    }
}
