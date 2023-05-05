use std::str::Utf8Error;
use std::time::Duration;

use dotenv_codegen::dotenv;
use futures::io::BufReader;
use futures::{AsyncBufReadExt, TryStreamExt};
use gloo::timers::future::sleep;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::request::{AskClippyRequest, ClippyContext};
use shared::response::{ChatErrorType, ChatUpdate, SearchResult};
use thiserror::Error;
use yew::html::Scope;

use crate::pages::search::{HistoryItem, HistorySource};
use crate::pages::search::{Msg, SearchPage};

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("HTTP request error: {0}")]
    HttpError(#[from] reqwest::Error),
    #[error("RequestError: {0}")]
    RequestError(#[from] serde_json::Error),
    #[error("Received malformed data: {0}")]
    StreamError(#[from] Utf8Error),
}

pub struct SpyglassClient {
    client: Client,
    lens: String,
    endpoint: String,
}

impl SpyglassClient {
    pub fn new(lens: String) -> Self {
        let client = Client::new();

        #[cfg(debug_assertions)]
        let endpoint = dotenv!("SPYGLASS_BACKEND_DEV");
        #[cfg(not(debug_assertions))]
        let endpoint = dotenv!("SPYGLASS_BACKEND_PROD");

        Self {
            client,
            lens,
            endpoint: endpoint.to_string(),
        }
    }

    pub async fn followup(
        &mut self,
        followup: &str,
        history: &[HistoryItem],
        doc_context: &[SearchResult],
        link: Scope<SearchPage>,
    ) -> Result<(), ClientError> {
        let mut context = history
            .iter()
            .filter(|x| x.source != HistorySource::System)
            .map(|x| ClippyContext::History(x.source.to_string(), x.value.clone()))
            .collect::<Vec<_>>();

        // Add urls to context
        for result in doc_context.iter() {
            context.push(ClippyContext::DocId(result.doc_id.clone()));
        }

        let body = AskClippyRequest {
            query: followup.to_string(),
            lens: Some(vec![self.lens.clone()]),
            context,
        };

        self.handle_request(&body, link.clone()).await
    }

    pub async fn search(
        &mut self,
        query: &str,
        link: Scope<SearchPage>,
    ) -> Result<(), ClientError> {
        let body = AskClippyRequest {
            query: query.to_string(),
            lens: Some(vec![self.lens.clone()]),
            context: Vec::new(),
        };

        self.handle_request(&body, link.clone()).await
    }

    async fn handle_request(
        &mut self,
        body: &AskClippyRequest,
        link: Scope<SearchPage>,
    ) -> Result<(), ClientError> {
        let url = format!("{}/chat", self.endpoint);

        let res = self
            .client
            .post(url)
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let res = res
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            .into_async_read();

        let mut reader = BufReader::new(res);
        let mut buf = String::new();
        while reader.read_line(&mut buf).await.is_ok() {
            let line = buf.trim_end_matches(|c| c == '\r' || c == '\n');
            let line = line.strip_prefix("data:").unwrap_or(line);
            if line.is_empty() {
                buf.clear();
                continue;
            }

            let update = serde_json::from_str::<ChatUpdate>(line)?;
            match update {
                ChatUpdate::SearchingDocuments => {
                    log::info!("ChatUpdate::SearchingDocuments");
                    link.send_message(Msg::SetStatus("Searching...".into()))
                }
                ChatUpdate::DocumentContextAdded(docs) => {
                    log::info!("ChatUpdate::DocumentContextAdded");
                    link.send_message(Msg::SetSearchResults(docs))
                }
                ChatUpdate::GeneratingContext => {
                    log::info!("ChatUpdate::SearchingDocuments");
                    link.send_message(Msg::SetStatus("Analyzing documents...".into()))
                }
                ChatUpdate::ContextGenerated(context) => {
                    log::info!("ChatUpdate::ContextGenerated {}", context);
                    link.send_message(Msg::ContextAdded(context));
                }
                ChatUpdate::LoadingModel | ChatUpdate::LoadingPrompt => {
                    link.send_message(Msg::SetStatus("Generating answer...".into()))
                }
                ChatUpdate::Token(token) => link.send_message(Msg::TokenReceived(token)),
                ChatUpdate::EndOfText => {
                    link.send_message(Msg::SetFinished);
                    break;
                }
                ChatUpdate::Error(err) => {
                    log::error!("ChatUpdate::Error: {err:?}");
                    let msg = match err {
                        ChatErrorType::ContextLengthExceeded(msg) => msg,
                        ChatErrorType::APIKeyMissing => "No API key".into(),
                        ChatErrorType::UnknownError(msg) => msg,
                    };
                    link.send_message(Msg::SetError(msg));
                    break;
                }
            }
            buf.clear();
            // give ui thread a chance to do something
            sleep(Duration::from_millis(50)).await;
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Lens {
    pub id: i64,
    pub name: String,
    pub display_name: String,
    pub example_questions: Vec<String>,
    pub example_docs: Vec<String>,
    pub is_public: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserData {
    pub lenses: Vec<Lens>,
}

pub async fn get_user_data(auth_token: &str) -> Result<UserData, reqwest::Error> {
    #[cfg(debug_assertions)]
    let endpoint = dotenv!("SPYGLASS_BACKEND_DEV");
    #[cfg(not(debug_assertions))]
    let endpoint = dotenv!("SPYGLASS_BACKEND_PROD");

    let client = reqwest::Client::new();
    let lenses = client
        .get(format!("{}/user/lenses", endpoint))
        .bearer_auth(auth_token)
        .send()
        .await?
        .json::<Vec<Lens>>()
        .await;

    let lenses = match lenses {
        Ok(lenses) => lenses,
        Err(err) => {
            log::error!("Unable to get lenses: {}", err.to_string());
            Vec::new()
        }
    };

    Ok(UserData { lenses })
}
