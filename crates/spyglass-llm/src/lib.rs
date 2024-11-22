use anyhow::Result;
use lazy_static::lazy_static;
use model::LLMModel;
use shared::llm::{ChatMessage, ChatRole, ChatStream, LlmSession};
use std::path::PathBuf;
use tera::{Context, Tera};

pub mod model;
pub mod sampler;
mod token_output_stream;

lazy_static! {
    pub static ref TEMPLATES: Tera = {
        match Tera::new("assets/templates/llm/*.txt") {
            Ok(t) => t,
            Err(err) => {
                eprintln!("Parsing error: {err}");
                ::std::process::exit(1);
            }
        }
    };
}

#[derive(Clone)]
pub struct LlmClient {
    llm: LLMModel,
}

impl LlmClient {
    pub fn new(gguf_path: PathBuf) -> Result<Self> {
        Ok(Self {
            llm: LLMModel::new(gguf_path)?,
        })
    }

    pub async fn chat(
        &mut self,
        session: &LlmSession,
        stream: Option<tokio::sync::mpsc::Sender<ChatStream>>,
    ) -> Result<ChatMessage> {
        // Encode the prompt.
        let mut all_tokens = vec![];
        let mut content_buffer = String::new();
        let mut sampler = self.llm.sampler();

        // process prompt
        let mut timer = std::time::Instant::now();
        if let Some(stream) = &stream {
            let _ = stream.send(ChatStream::LoadingPrompt).await;
        }

        let prompt_contents =
            TEMPLATES.render("llama3-instruct.txt", &Context::from_serialize(session)?)?;
        let next_token = sampler.load_prompt(&prompt_contents)?;
        log::info!("processing prompt in {:.3}s", timer.elapsed().as_secs_f32());

        if let Some(stream) = &stream {
            let _ = stream.send(ChatStream::ChatStart).await;
        }

        all_tokens.push(next_token);
        if let Some(t) = self.llm.stream.next_token(next_token)? {
            content_buffer.push_str(&t);
            if let Some(stream) = &stream {
                let _ = stream.send(ChatStream::Token(t)).await;
            }
        }

        timer = std::time::Instant::now();
        let mut sampled = 1;
        let num_tokens_to_sample = 1024;

        for _ in 0..num_tokens_to_sample {
            let next_token = sampler.next_token()?;
            all_tokens.push(next_token);
            if let Some(t) = self.llm.stream.next_token(next_token)? {
                content_buffer.push_str(&t);
                if let Some(stream) = &stream {
                    let _ = stream.send(ChatStream::Token(t)).await;
                }
            }

            sampled += 1;
            if sampler.is_done() {
                break;
            };
        }

        if let Some(rest) = self.llm.stream.decode_rest().map_err(candle::Error::msg)? {
            if let Some(stream) = &stream {
                let _ = stream.send(ChatStream::Token(rest)).await;
            }
        }

        if let Some(stream) = &stream {
            let _ = stream.send(ChatStream::ChatDone).await;
        }

        log::info!(
            "{sampled:4} tokens generated: {:.2} token/s",
            sampled as f64 / timer.elapsed().as_secs_f64(),
        );

        Ok(ChatMessage {
            role: ChatRole::Assistant,
            content: content_buffer,
        })
    }
}
