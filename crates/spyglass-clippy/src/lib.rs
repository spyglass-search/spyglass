use std::{convert::Infallible, io::Write};

use llama_rs::{
    InferenceParameters, InferenceSessionParameters, LoadError, LoadProgress, ModelKVMemoryType,
    TokenBias,
};
use rand::SeedableRng;

const MODEL_PATH: &str = "../../assets/models/alpaca-native.7b.bin";

fn construct_prompt(prompt: &str, doc_context: Option<Vec<String>>) -> String {
    // Begin the template in the form alpaca expects
    let mut start: String = r#"
Below is an instruction that describes a task. Write a response that appropriately completes the request.
### Instruction:
"#.into();
    // Add any additional context for the LLM to use.
    if let Some(ctxt) = doc_context {
        start.push_str("Using info from the following text: ");
        for data in ctxt {
            start.push_str(&data);
            start.push('\n');
        }
    }

    // Add the user question/prompt
    start.push_str(prompt);
    // wrap it up
    start.push_str("\n### Response:\n");

    start
}

pub async fn unleash_clippy(
    prompt: &str,
    doc_context: Option<Vec<String>>,
) -> Result<(), LoadError> {
    let prompt = construct_prompt(prompt, doc_context);

    let inference_params = InferenceParameters {
        n_threads: 8,
        n_batch: 8,
        top_k: 40,
        top_p: 0.5,
        repeat_penalty: 1.17647,
        temp: 0.7,
        bias_tokens: TokenBias::default(),
        play_back_previous_tokens: false,
        ..Default::default()
    };

    let inference_session_params = {
        let mem_typ = ModelKVMemoryType::Float16;
        InferenceSessionParameters {
            memory_k_type: mem_typ,
            memory_v_type: mem_typ,
            repetition_penalty_last_n: 16,
        }
    };

    let (model, vocab) = llama_rs::Model::load(MODEL_PATH, 2048, |progress| match progress {
        LoadProgress::HyperparametersLoaded(hparams) => {
            println!("Loaded HyperParams {hparams:#?}")
        }
        LoadProgress::ContextSize { bytes } => println!(
            "ggml ctx size = {:.2} MB\n",
            bytes as f64 / (1024.0 * 1024.0)
        ),
        LoadProgress::MemorySize { bytes, n_mem } => println!(
            "Memory size: {} MB {}",
            bytes as f32 / 1024.0 / 1024.0,
            n_mem
        ),
        LoadProgress::PartTensorLoaded {
            file: _,
            current_tensor,
            tensor_count,
        } => {
            if current_tensor % 20 == 0 || current_tensor == tensor_count {
                let percent = ((current_tensor as f32 * 100f32) / tensor_count as f32) as u8;
                println!("{}/{} ({}%)", current_tensor, tensor_count, percent);
            }
        }
        _ => {}
    })?;

    println!("Model loaded fully");
    let mut rng = rand::rngs::StdRng::from_entropy();
    let mut session = model.start_session(inference_session_params);

    let res = session.inference_with_prompt::<Infallible>(
        &model,
        &vocab,
        &inference_params,
        &prompt,
        Some(2048),
        &mut rng,
        |t| {
            print!("{t}");
            std::io::stdout().flush().unwrap();
            Ok(())
        },
    );

    match res {
        Ok(_) => (),
        Err(llama_rs::InferenceError::ContextFull) => {
            println!("Context window full, stopping inference.")
        }
        Err(llama_rs::InferenceError::TokenizationFailed) => {
            println!("Failed to tokenize initial prompt.");
        }
        Err(llama_rs::InferenceError::UserCallback(_)) => unreachable!("cannot fail"),
    }

    Ok(())
}

#[cfg(test)]
mod test {
    #[tokio::test]
    pub async fn test_basic_prompt() {
        super::unleash_clippy("how much water should you drink daily?", None)
            .await
            .expect("Unable to prompt");
    }

    #[ignore]
    #[tokio::test]
    pub async fn test_prompt_with_data() {
        let data: Vec<String> = vec![
            "Instruction-following models such as GPT-3.5 (text-davinci-003), ChatGPT, Claude, and Bing Chat have become increasingly powerful. Many users now interact with these models regularly and even use them for work. However, despite their widespread deployment, instruction-following models still have many deficiencies: they can generate false information, propagate social stereotypes, and produce toxic language.".into(),
            "To make maximum progress on addressing these pressing problems, it is important for the academic community to engage. Unfortunately, doing research on instruction-following models in academia has been difficult, as there is no easily accessible model that comes close in capabilities to closed-source models such as OpenAI’s text-davinci-003.".into(),
            "We are releasing our findings about an instruction-following language model, dubbed Alpaca, which is fine-tuned from Meta’s LLaMA 7B model. We train the Alpaca model on 52K instruction-following demonstrations generated in the style of self-instruct using text-davinci-003. On the self-instruct evaluation set, Alpaca shows many behaviors similar to OpenAI’s text-davinci-003, but is also surprisingly small and easy/cheap to reproduce.".into(),
            "We are releasing our training recipe and data, and intend to release the model weights in the future. We are also hosting an interactive demo to enable the research community to better understand the behavior of Alpaca. Interaction can expose unexpected capabilities and failures, which will guide us for the future evaluation of these models. We also encourage users to report any concerning behaviors in our web demo so that we can better understand and mitigate these behaviors. As any release carries risks, we discuss our thought process for this open release later in this blog post.".into(),
            "We emphasize that Alpaca is intended only for academic research and any commercial use is prohibited. There are three factors in this decision: First, Alpaca is based on LLaMA, which has a non-commercial license, so we necessarily inherit this decision. Second, the instruction data is based on OpenAI’s text-davinci-003, whose terms of use prohibit developing models that compete with OpenAI. Finally, we have not designed adequate safety measures, so Alpaca is not ready to be deployed for general use.".into(),
        ];
        super::unleash_clippy("what is alpaca?", Some(data))
            .await
            .expect("Unable to prompt");
    }
}
