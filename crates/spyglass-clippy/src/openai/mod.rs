use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::request::{AskClippyRequest, ClippyContext};
use std::{
    convert::Infallible,
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::mpsc;

use crate::ChatUpdate;

#[derive(Serialize, Debug)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize, Debug)]
struct CompletionRequest<'a> {
    max_tokens: i32,
    n: i32,
    temperature: f32,
    frequency_penalty: f32,
    presence_penalty: f32,
    stop: Option<String>,
    model: &'a str,
    messages: Vec<ChatMessage>,
    stream: bool,
}

#[derive(Serialize, Deserialize, Debug)]
struct ChatCompletion {
    id: String,
    object: String,
    created: i64,
    model: String,
    usage: Usage,
    choices: Vec<Choice>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Usage {
    prompt_tokens: i32,
    completion_tokens: i32,
    total_tokens: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Choice {
    message: Message,
    finish_reason: String,
    index: i32,
}

#[derive(Serialize, Deserialize, Debug)]
struct Message {
    role: String,
    content: String,
}

pub async fn process_question(
    api_key: &str,
    stream: mpsc::UnboundedSender<ChatUpdate>,
    context: &str,
    query: AskClippyRequest,
) -> Result<(), reqwest::Error> {
    let chat = build_chat(context, &query);
    let _ = stream.send(ChatUpdate::LoadingModel);
    log::debug!("Chat {:?}", chat);
    let request_body = CompletionRequest {
        max_tokens: 100,
        n: 1,
        temperature: 1.0,
        frequency_penalty: 0.0,
        presence_penalty: 0.0,
        stop: None,
        model: "gpt-3.5-turbo",
        messages: chat,
        stream: false,
    };
    let client = Client::new();
    let response = client
        .post(&format!("https://api.openai.com/v1/chat/completions"))
        .header("Content-Type", "application/json")
        .header("Authorization", &format!("Bearer {}", api_key))
        .json(&request_body)
        .send()
        .await?;

    log::debug!("Response {:?}", response);
    let response: ChatCompletion = response.json().await?;

    for choice in response.choices {
        log::debug!("Answer {:?}", choice);
        let answer = choice.message.content.trim().to_string();
        stream.send(ChatUpdate::Token(answer));
    }

    stream.send(ChatUpdate::EndOfText);
    Ok(())
}

fn build_chat(context: &str, query: &AskClippyRequest) -> Vec<ChatMessage> {
    let mut message = Vec::new();
    message.push(ChatMessage {
        role: String::from("system"),
        content: String::from("You are a research assistant who uses provided research CONTEXT and your own knowledge to answer questions as factual as possible")
    });

    let mut chat_history = Vec::new();
    let mut doc_ids = Vec::new();
    for context in &query.context {
        match context {
            ClippyContext::DocId(doc_id) => doc_ids.push(doc_id.clone()),
            ClippyContext::History(role, content) => chat_history.push(ChatMessage {
                role: role.clone(),
                content: content.clone(),
            }),
        }
    }
    message.push(ChatMessage {
        role: String::from("user"),
        content: String::from(context),
    });

    message.push(ChatMessage {
        role: String::from("user"),
        content: query.query.clone(),
    });
    message
}
