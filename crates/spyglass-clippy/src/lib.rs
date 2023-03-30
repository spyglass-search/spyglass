use std::{
    convert::Infallible,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;

use llama_rs::{
    InferenceParameters, InferenceSessionParameters, LoadError, LoadProgress, ModelKVMemoryType,
    OutputToken, TokenBias,
};
use rand::SeedableRng;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenResult {
    LoadingModel,
    LoadingPrompt,
    EndOfText,
    Error(String),
    Token(String),
}

fn construct_prompt(prompt: &str, doc_context: Option<Vec<String>>) -> String {
    // Begin the template in the form alpaca expects
    let mut start: String = if doc_context.is_some() {
        r#"Below is an instruction that describes a task, paired with an input that provides further context. Write a response that appropriately completes the request.
### Instruction:
"#.into()
    } else {
        r#"Below is an instruction that describes a task. Write a response that appropriately completes the request.
### Instruction:
"#.into()
    };

    // Add the user question/prompt
    start.push_str(prompt);

    // Add any additional context for the LLM to use.
    if let Some(ctxt) = doc_context {
        start.push_str("\n### Input:\n");
        for data in ctxt {
            start.push_str(&data);
            start.push('\n');
        }
    }

    // wrap it up
    start.push_str("\n### Response:\n");

    start
}

pub fn unleash_clippy(
    // Path to LLM model
    model: PathBuf,
    // Channel where generated tokens will be sent
    stream: mpsc::UnboundedSender<TokenResult>,
    // User prompt
    prompt: &str,
    // Any additional context we want to add to the prompt template
    doc_context: Option<Vec<String>>,
    // Whether or not the stream should include the prompt
    output_prompt: bool,
) -> Result<(), LoadError> {
    let handle = tokio::runtime::Handle::current();
    let prompt = prompt.to_string();
    // Spawns a new thread for inference so we don't block the ui/other tasks
    std::thread::spawn(move || {
        let model = model.clone();
        let stream = stream.clone();
        let prompt = prompt.to_string();
        let doc_context = doc_context.clone();
        // Now we spawn using tokio so that async sends using the channel are handled
        // correctly.
        handle.spawn_blocking(move || {
            let _ = stream.send(TokenResult::LoadingModel);
            run_model(model, stream.clone(), &prompt, doc_context, output_prompt)
                .expect("unable to prompt")
        });
    });

    Ok(())
}

fn run_model(
    model_path: PathBuf,
    stream: mpsc::UnboundedSender<TokenResult>,
    prompt: &str,
    doc_context: Option<Vec<String>>,
    output_prompt: bool,
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

    let (model, vocab) =
        llama_rs::Model::load(model_path.clone(), 2048, |progress| match progress {
            LoadProgress::HyperparametersLoaded(hparams) => {
                log::debug!("Loaded HyperParams {hparams:#?}")
            }
            LoadProgress::ContextSize { bytes } => log::debug!(
                "ggml ctx size = {:.2} MB\n",
                bytes as f64 / (1024.0 * 1024.0)
            ),
            LoadProgress::MemorySize { bytes, n_mem } => log::debug!(
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
                    log::debug!("{}/{} ({}%)", current_tensor, tensor_count, percent);
                }
            }
            _ => {}
        })?;

    log::debug!("`{:?}` model loaded", model_path.display());
    let tx = stream.clone();
    tokio::spawn(async move {
        let _ = tx.send(TokenResult::LoadingPrompt);
    });

    let mut rng = rand::rngs::StdRng::from_entropy();
    let mut session = model.start_session(inference_session_params);

    let tx = stream.clone();

    let prompt_check = Arc::new(Mutex::new(String::new()));
    let og_prompt = prompt.to_string();
    let res = session.inference_with_prompt::<Infallible>(
        &model,
        &vocab,
        &inference_params,
        &prompt,
        Some(2048),
        &mut rng,
        move |t| {
            let pcheck = prompt_check.clone();
            match t {
                OutputToken::Token(c) => {
                    let c = c.to_string();
                    let tx = tx.clone();
                    if output_prompt {
                        tokio::spawn(async move {
                            let _ = tx.send(TokenResult::Token(c.to_string()));
                        });
                    } else if let Ok(mut pcheck) = pcheck.lock() {
                        // detect whether the model has finished inputting the prompt to the model
                        if *pcheck == og_prompt {
                            tokio::spawn(async move {
                                let _ = tx.send(TokenResult::Token(c.to_string()));
                            });
                        } else {
                            pcheck.push_str(&c);
                        }
                    }
                }
                OutputToken::EndOfText => {
                    let tx = tx.clone();
                    tokio::spawn(async move {
                        let _ = tx.send(TokenResult::EndOfText);
                    });
                }
            }
            Ok(())
        },
    );

    match res {
        Ok(_) => (),
        Err(llama_rs::InferenceError::ContextFull) => {
            let _ = stream.send(TokenResult::Error(
                "Context window full, stopping inference.".into(),
            ));
        }
        Err(llama_rs::InferenceError::TokenizationFailed) => {
            let _ = stream.send(TokenResult::Error(
                "Failed to tokenize initial prompt.".into(),
            ));
        }
        Err(llama_rs::InferenceError::UserCallback(_)) => unreachable!("cannot fail"),
    }

    tokio::spawn(async move {
        stream.closed().await;
    });
    Ok(())
}

#[cfg(test)]
mod test {
    use std::io::Write;

    use crate::TokenResult;
    use tokio::sync::mpsc;

    const MODEL_PATH: &str = "../../assets/models/alpaca-native.7b.bin";

    #[tokio::test]
    pub async fn test_basic_prompt() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let recv_task = tokio::spawn(async move {
            let mut generated = String::new();
            while let Some(msg) = rx.recv().await {
                match msg {
                    TokenResult::Token(c) => {
                        print!("{}", c.clone());
                        std::io::stdout().flush().unwrap();
                        generated.push_str(&c)
                    }
                    TokenResult::Error(msg) => {
                        eprintln!("Received an error: {}", msg);
                        break;
                    }
                    TokenResult::EndOfText => break,
                    _ => {}
                }
            }
        });

        tokio::spawn(async move {
            super::unleash_clippy(
                MODEL_PATH.into(),
                tx.clone(),
                "what is the difference between an alpaca & llama?",
                None,
                true,
            )
            .expect("unable to prompt");
        });

        let _ = recv_task.await;
    }

    #[ignore]
    #[tokio::test]
    pub async fn test_prompt_with_data() {
        let (tx, mut rx) = mpsc::unbounded_channel();

        let data: Vec<String> = vec![
            r#"Background Cuno is a foul-mouthed, speed-addicted youth first encountered throwing rocks at the hanged man's corpse. In response to his father's violence - who he lives with in the Capeside apartments up until just before the events of the game - Cuno developed a respect for brutality and power, as well as an infatuation with the RCM, seeking their validation[3] It may have also inspired him to abandon his "lame" legal name for his nickname "Cuno", a name reminiscent of a "primal" force or "rabid dog".[4][5] Having dropped out of school two years prior, Cuno now spends his days exploring the derelict city, vandalizing public property,[6] building up his shack, and selling FALN gear on the side. He also steals goods from the lorries.[7] Although he denies it repeatedly, Cuno enjoys reading the Man from Hjelmdall series.[8][9] He also listens to various radio stations,[10] including snuff radio.[11] Cuno knows a great deal about Martinaise and its inhabitants, and they know and dislike him, too. He has a crush on Lilienne.[12] Cuno addresses himself in third-person as a defense mechanism.[13]"#.into()
        ];
        super::unleash_clippy(
            MODEL_PATH.into(),
            tx.clone(),
            "where does cuno live?",
            Some(data),
            true,
        )
        .expect("Unable to prompt");

        let mut generated = String::new();
        while let Some(msg) = rx.recv().await {
            match msg {
                TokenResult::Token(c) => generated.push_str(&c),
                TokenResult::Error(msg) => eprintln!("Received an error: {}", msg),
                TokenResult::EndOfText => {}
                _ => {}
            }
        }
    }
}
