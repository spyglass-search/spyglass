use std::str::Utf8Error;
use std::time::Duration;

use dotenv_codegen::dotenv;
use futures::io::BufReader;
use futures::{select, AsyncBufReadExt, FutureExt, StreamExt, TryStreamExt};
use gloo::file::{Blob, ObjectUrl};
use gloo::timers::future::sleep;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::request::{AskClippyRequest, ClippyContext};
use shared::response::{ChatUpdate, SearchResult};
use thiserror::Error;
use yew::platform::pinned::mpsc::UnboundedReceiver;

use crate::components::file_upload::FileDetails;
use crate::pages::search::{HistoryItem, HistorySource, WorkerCmd};
use crate::schema::{
    EmbedConfiguration, GetLensSourceRequest, GetLensSourceResponse, LensSourceQueryFilter,
};
use reqwest::multipart::Part;

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

pub type ChatUpdateCallback = dyn Fn(ChatUpdate);

pub struct SpyglassClient {
    client: Client,
    lens: String,
    endpoint: String,
    auth: Option<String>,
    session_uuid: String,
    public_api: bool,
}

impl SpyglassClient {
    pub fn new(lens: String, session_uuid: String, auth: Option<String>, public_api: bool) -> Self {
        let client = Client::new();

        #[cfg(debug_assertions)]
        let endpoint = dotenv!("SPYGLASS_BACKEND_DEV");
        #[cfg(not(debug_assertions))]
        let endpoint = dotenv!("SPYGLASS_BACKEND_PROD");

        Self {
            client,
            lens,
            endpoint: endpoint.to_string(),
            auth,
            session_uuid,
            public_api,
        }
    }

    pub async fn followup(
        &mut self,
        followup: &str,
        history: &[HistoryItem],
        doc_context: &[SearchResult],
        chat_uuid: &Option<String>,
        callback: &ChatUpdateCallback,
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
            request_uuid: chat_uuid.clone(),
        };

        self.handle_request(&body, callback, channel).await
    }

    pub async fn search(
        &mut self,
        query: &str,
        callback: &ChatUpdateCallback,
        channel: UnboundedReceiver<WorkerCmd>,
    ) -> Result<(), ClientError> {
        let body = AskClippyRequest {
            query: query.to_string(),
            lens: Some(vec![self.lens.clone()]),
            context: Vec::new(),
            request_uuid: None,
        };

        self.handle_request(&body, callback, channel).await
    }

    async fn handle_request(
        &mut self,
        body: &AskClippyRequest,
        callback: &ChatUpdateCallback,
        mut channel: UnboundedReceiver<WorkerCmd>,
    ) -> Result<(), ClientError> {
        let url = if self.public_api {
            format!("{}/api/v1/chat", self.endpoint)
        } else {
            format!("{}/chat", self.endpoint)
        };

        let mut request = self.client.post(url).body(serde_json::to_string(&body)?);

        if let Some(auth_token) = &self.auth {
            request = request.bearer_auth(auth_token);
        }
        request = request.header("spyglass-session", self.session_uuid.clone());

        let resp = request.send().await?;

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
                    callback(update.clone());
                    match update {
                        ChatUpdate::Error(_) | ChatUpdate::EndOfText => {
                            break;
                        }
                        _ => ()
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
    pub image: Option<String>,
    pub description: Option<String>,
    pub embedded_configuration: Option<EmbedConfiguration>,
}

/// Chat history for a single chat session
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct ChatHistoryEntry {
    pub qna: Vec<QuestionAndAnswer>,
    pub lenses: Vec<String>,
    pub session_id: String,
    pub last_updated_at: String,
}

/// Individual question and answer for a Chat
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct QuestionAndAnswer {
    pub question: String,
    pub response: String,
    pub successful: bool,
    pub created_at: String,
    pub documents: Vec<String>,
    pub document_details: Option<Vec<SearchResult>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserData {
    pub lenses: Vec<Lens>,
    pub history: Vec<ChatHistoryEntry>,
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreferredModel {
    WhisperSmall,
    WhisperMedium,
    WhisperLarge,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum LensAddDocType {
    Audio,
    /// Token is used to download the document from GDrive.
    GDrive {
        token: String,
    },
    RssFeed {
        preferred_model: PreferredModel,
    },
    /// Normal, web accessible URL.
    WebUrl {
        include_all_suburls: bool,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct LensAddDocument {
    pub doc_type: LensAddDocType,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub enum LensDocType {
    Audio,
    GDrive,
    Web,
    Upload,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LensSource {
    pub display_name: String,
    pub doc_type: LensDocType,
    pub url: String,
    pub status: String,
    pub doc_uuid: String,
}

#[derive(Deserialize)]
pub struct SourceValidationResponse {
    pub url: String,
    pub url_count: u64,
    pub has_sitemap: Option<bool>,
    pub is_valid: bool,
    pub validation_msg: Option<String>,
}

#[derive(Error, Debug)]
pub enum ApiError {
    #[error("You need to sign in.")]
    Unauthorized,
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
    public_api: bool,
}

impl ApiClient {
    pub fn new(token: Option<String>, public_api: bool) -> Self {
        #[cfg(debug_assertions)]
        let endpoint: String = dotenv!("SPYGLASS_BACKEND_DEV").into();
        #[cfg(not(debug_assertions))]
        let endpoint: String = dotenv!("SPYGLASS_BACKEND_PROD").into();

        let client = reqwest::Client::new();

        Self {
            client,
            endpoint,
            token,
            public_api,
        }
    }

    pub async fn lens_create(&self) -> Result<Lens, ApiError> {
        let mut request = self.client.post(format!("{}/user/lenses", self.endpoint));
        if let Some(auth_token) = &self.token {
            request = request.bearer_auth(auth_token);
        }

        Ok(request.send().await?.json::<Lens>().await?)
    }

    /// Creates a local in memory object url that represent a downloaded file.
    pub async fn download_file(&self, url: &str) -> Result<ObjectUrl, ApiError> {
        let mut request = self
            .client
            .get(format!("{}/user/lenses{}", self.endpoint, url));

        if let Some(auth_token) = &self.token {
            request = request.bearer_auth(auth_token);
        }

        let response = request.send().await;
        match response {
            Ok(rsp) => {
                let bytes = rsp.bytes().await.unwrap();
                let byte_vec = bytes.to_vec();
                let blob = Blob::new(byte_vec.as_slice());
                let url = ObjectUrl::from(blob);
                Ok(url)
            }
            Err(error) => {
                log::error!("Error requesting download {:?}", error);
                Err(ApiError::Unauthorized)
            }
        }
    }

    /// Deletes the specified lens. This will delete the lens and all associated
    /// stored information
    pub async fn lens_delete(&self, lens: &str) -> Result<Lens, ApiError> {
        let mut request = self
            .client
            .delete(format!("{}/user/lenses/{}", self.endpoint, lens));
        if let Some(auth_token) = &self.token {
            request = request.bearer_auth(auth_token);
        }

        Ok(request.send().await?.json::<Lens>().await?)
    }

    pub async fn lens_retrieve(&self, id: &str) -> Result<Lens, ApiError> {
        let url = if self.public_api {
            format!("{}/api/v1/lenses/{}", self.endpoint, id)
        } else {
            format!("{}/user/lenses/{}", self.endpoint, id)
        };
        let mut request = self.client.get(url);
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

    pub async fn lens_retrieve_sources(
        &self,
        id: &str,
        page: usize,
        filter: LensSourceQueryFilter,
    ) -> Result<GetLensSourceResponse, ApiError> {
        let req = GetLensSourceRequest { page, filter };

        match &self.token {
            Some(token) => Ok(self
                .client
                .get(format!("{}/user/lenses/{}/sources", self.endpoint, id))
                .query(&req)
                .bearer_auth(token)
                .send()
                .await?
                .error_for_status()?
                .json::<GetLensSourceResponse>()
                .await?),
            None => Err(ApiError::Unauthorized),
        }
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

    /// Uploads a document source for the specified lens.
    pub async fn upload_source_document(
        &self,
        lens: &str,
        file_info: Box<FileDetails>,
    ) -> Result<(), ApiError> {
        match &self.token {
            Some(token) => {
                let data = file_info.data;
                let name = file_info.name;
                let file_type = file_info.file_type;
                let mut part = Part::bytes(data);
                part = part.file_name(name.clone());
                part = part.mime_str(&file_type)?;

                let form = reqwest::multipart::Form::new().part(name.clone(), part);

                let resp = self
                    .client
                    .post(format!("{}/user/lenses/{}/upload", self.endpoint, lens))
                    .bearer_auth(token)
                    .multipart(form)
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

    /// Deletes the specified lens source from the specified lens
    pub async fn delete_lens_source(&self, lens: &str, source_uuid: &str) -> Result<(), ApiError> {
        match &self.token {
            Some(token) => {
                let resp = self
                    .client
                    .delete(format!(
                        "{}/user/lenses/{}/source/{}",
                        self.endpoint, lens, source_uuid
                    ))
                    .bearer_auth(token)
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
            None => Err(ApiError::Unauthorized),
        }
    }

    pub async fn validate_lens_source(
        &self,
        lens: &str,
        request: &LensAddDocument,
    ) -> Result<SourceValidationResponse, ApiError> {
        match &self.token {
            Some(token) => {
                let resp = self
                    .client
                    .post(format!(
                        "{}/user/lenses/{}/validate/source",
                        self.endpoint, lens
                    ))
                    .bearer_auth(token)
                    .json(request)
                    .send()
                    .await?;

                match resp.error_for_status_ref() {
                    Ok(_) => match resp.json::<SourceValidationResponse>().await {
                        Ok(response) => Ok(response),
                        Err(msg) => Err(ApiError::Other(msg.to_string())),
                    },
                    Err(err) => match resp.json::<ApiErrorMessage>().await {
                        Ok(msg) => Err(ApiError::ClientError(msg)),
                        Err(_) => Err(ApiError::RequestError(err)),
                    },
                }
            }
            None => Err(ApiError::Unauthorized),
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
        match &self.token {
            Some(token) => {
                let request = self
                    .client
                    .get(format!("{}/user/lenses", self.endpoint))
                    .bearer_auth(token)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<Vec<Lens>>()
                    .await;

                let lenses = match request {
                    Ok(lenses) => lenses,
                    Err(err) => {
                        log::error!("Unable to get lenses: {}", err.to_string());
                        Vec::new()
                    }
                };

                let history_response = self
                    .client
                    .get(format!("{}/user/chat/history", self.endpoint))
                    .bearer_auth(token)
                    .send()
                    .await?
                    .error_for_status()?
                    .json::<Vec<ChatHistoryEntry>>()
                    .await;
                let history = match history_response {
                    Ok(history_val) => history_val,
                    Err(err) => {
                        log::error!("Unable to access chat history: {}", err.to_string());
                        Vec::new()
                    }
                };

                Ok(UserData { lenses, history })
            }
            None => {
                log::error!("User is not logged in");
                Ok(UserData {
                    lenses: Vec::new(),
                    history: Vec::new(),
                })
            }
        }
    }
}
