use std::str::Utf8Error;
use std::time::Duration;

use dotenv_codegen::dotenv;
use futures::io::BufReader;
use futures::{select, AsyncBufReadExt, FutureExt, StreamExt, TryStreamExt};
use gloo::timers::future::sleep;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::request::{AskClippyRequest, ClippyContext};
use shared::response::{ChatErrorType, ChatUpdate, SearchResult};
use thiserror::Error;
use yew::html::Scope;
use yew::platform::pinned::mpsc::UnboundedReceiver;

use crate::pages::search::{HistoryItem, HistorySource, WorkerCmd};
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
        channel: UnboundedReceiver<WorkerCmd>,
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

        self.handle_request(&body, link.clone(), channel).await
    }

    pub async fn search(
        &mut self,
        query: &str,
        link: Scope<SearchPage>,
        channel: UnboundedReceiver<WorkerCmd>,
    ) -> Result<(), ClientError> {
        let body = AskClippyRequest {
            query: query.to_string(),
            lens: Some(vec![self.lens.clone()]),
            context: Vec::new(),
        };

        self.handle_request(&body, link.clone(), channel).await
    }

    async fn handle_request(
        &mut self,
        body: &AskClippyRequest,
        link: Scope<SearchPage>,
        mut channel: UnboundedReceiver<WorkerCmd>,
    ) -> Result<(), ClientError> {
        let url = format!("{}/chat", self.endpoint);

        let resp = self
            .client
            .post(url)
            .body(serde_json::to_string(&body)?)
            .send()
            .await?;

        let res = resp
            .bytes_stream()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))
            .into_async_read();

        let mut reader = BufReader::new(res);
        let mut buf = String::new();

        loop {
            let mut read_line = reader.read_line(&mut buf).fuse();
            select! {
                res = read_line => {
                    if res.is_err() {
                        break;
                    }

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
                _ = channel.next() => {
                    // aborting response generation
                    drop(reader);
                    break;
                }
            }
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
    pub sources: Vec<LensSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserData {
    pub lenses: Vec<Lens>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LensAddDocType {
    /// Token is used to download the document from GDrive.
    GDrive { token: String },
    /// Normal, web accessible URL.
    WebUrl,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LensAddDocument {
    pub doc_type: LensAddDocType,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum LensDocType {
    GDrive,
    Web,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LensSource {
    pub display_name: String,
    pub doc_type: LensDocType,
    pub url: String,
    pub status: String,
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Unable to make request: {0}")]
    RequestError(#[from] reqwest::Error),
    #[error("Api Error: {0}")]
    ClientError(ApiErrorMessage),
    #[error("Unable to make request: {0}")]
    Other(String),
}

#[derive(Clone, Deserialize, Debug)]
pub struct ApiErrorMessage {
    pub code: u16,
    pub message: String,
}

impl std::fmt::Display for ApiErrorMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("({}) {}", self.code, self.message))
    }
}

pub struct ApiClient {
    client: reqwest::Client,
    endpoint: String,
    token: Option<String>,
}

impl ApiClient {
    pub fn new(token: Option<String>) -> Self {
        #[cfg(debug_assertions)]
        let endpoint: String = dotenv!("SPYGLASS_BACKEND_DEV").into();
        #[cfg(not(debug_assertions))]
        let endpoint: String = dotenv!("SPYGLASS_BACKEND_PROD").into();

        let client = reqwest::Client::new();

        Self {
            client,
            endpoint,
            token,
        }
    }

    pub async fn lens_create(&self) -> Result<Lens, ApiError> {
        let mut request = self.client.post(format!("{}/user/lenses", self.endpoint));
        if let Some(auth_token) = &self.token {
            request = request.bearer_auth(auth_token);
        }

        Ok(request.send().await?.json::<Lens>().await?)
    }

    pub async fn lens_retrieve(&self, id: &str) -> Result<Lens, ApiError> {
        let mut request = self
            .client
            .get(format!("{}/user/lenses/{}", self.endpoint, id));
        if let Some(auth_token) = &self.token {
            request = request.bearer_auth(auth_token);
        }

        Ok(request
            .send()
            .await?
            .error_for_status()?
            .json::<Lens>()
            .await?)
    }

    pub async fn lens_add_source(
        &self,
        lens: &str,
        request: &LensAddDocument,
    ) -> Result<(), ApiError> {
        match &self.token {
            Some(token) => {
                let resp = self
                    .client
                    .post(format!("{}/user/lenses/{}/source", self.endpoint, lens))
                    .bearer_auth(token)
                    .json(request)
                    .send()
                    .await?;

                match resp.error_for_status_ref() {
                    Ok(_) => Ok(()),
                    Err(err) => match resp.json::<ApiErrorMessage>().await {
                        Ok(msg) => Err(ApiError::ClientError(msg)),
                        Err(_) => Err(ApiError::RequestError(err)),
                    },
                }
            }
            None => Ok(()),
        }
    }

    pub async fn lens_update(&self, lens: &str, display_name: &str) -> Result<(), ApiError> {
        match &self.token {
            Some(token) => {
                match self
                    .client
                    .patch(format!("{}/user/lenses/{}", self.endpoint, lens))
                    .bearer_auth(token)
                    .json(&serde_json::json!({ "display_name": display_name }))
                    .send()
                    .await?
                    .error_for_status()
                {
                    Ok(_) => Ok(()),
                    Err(err) => Err(ApiError::RequestError(err)),
                }
            }
            None => Ok(()),
        }
    }

    pub async fn get_user_data(&self) -> Result<UserData, ApiError> {
        let mut request = self.client.get(format!("{}/user/lenses", self.endpoint));
        if let Some(auth_token) = &self.token {
            request = request.bearer_auth(auth_token);
        }

        let lenses = request
            .send()
            .await?
            .error_for_status()?
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
}
