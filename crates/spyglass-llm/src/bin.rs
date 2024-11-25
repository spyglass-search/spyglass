use shared::llm::{ChatMessage, ChatRole, ChatStream, LlmSession};
use spyglass_llm::LlmClient;
/// This is mainly for testing the llm client, should remove this after it's comfortably
/// integrated with the other code.
///
use std::io::Write;
use tokio::sync::mpsc;

#[tokio::main]
pub async fn main() -> Result<(), anyhow::Error> {
    // Default to info logging if nothing is set.
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    pretty_env_logger::init();

    let prompt = LlmSession {
        messages: vec![
            ChatMessage {
                role: ChatRole::System,
                content: "You are a helpful AI assistant".into(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: "What is the capital of Zimbabwe?".into(),
            },
        ],
    };

    let (tx, mut rx) = mpsc::channel(10);
    // Spawn a task to stream the chat resp
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            match msg {
                ChatStream::LoadingPrompt => {
                    log::info!("loading prompt...");
                }
                ChatStream::ChatStart => {
                    log::info!("starting generation...");
                }
                ChatStream::Token(tok) => {
                    print!("{tok}");
                    std::io::stdout().flush().unwrap();
                }
                ChatStream::ChatDone => {
                    println!("🤖");
                    log::info!("DONE!");
                }
            }
        }
    });

    match LlmClient::new("assets/models/llm/llama3/Llama-3.2-3B-Instruct.Q5_K_M.gguf".into()) {
        Ok(mut client) => {
            client.chat(&prompt, Some(tx)).await?;
        }
        Err(error) => {
            log::error!("Error loading model {error}");
        }
    }

    Ok(())
}
