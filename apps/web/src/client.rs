use std::str::Utf8Error;

use futures::io::BufReader;
use futures::{AsyncBufReadExt, TryStreamExt};
use reqwest::Client;
use shared::request::AskClippyRequest;
use shared::response::ChatUpdate;
use thiserror::Error;
use yew::html::Scope;

use crate::{
    constants,
    pages::search::SearchPage,
};

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
}

impl SpyglassClient {
    pub fn new() -> Self {
        let client = Client::new();
        Self { client }
    }

    pub async fn search(
        &mut self,
        query: &str,
        _link: Scope<SearchPage>,
    ) -> Result<(), ClientError> {
        let url = format!("{}/chat", constants::HTTP_ENDPOINT);
        let body = AskClippyRequest {
            query: query.to_string(),
            lens: None,
            context: Vec::new(),
        };

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
        while let Ok(_) = reader.read_line(&mut buf).await {
            let line = buf.trim_end_matches(|c| c == '\r' || c == '\n');
            let line = line.strip_prefix("data:").unwrap_or(line);
            if line.len() == 0 {
                buf.clear();
                continue
            }

            let update = serde_json::from_str::<ChatUpdate>(&line)?;
            log::info!("update: {:?}", update);
            // match serde_json::from_str::<ChatUpdate>(token)? {
            //     ChatUpdate::SearchingDocuments => {

            //     },
            //     _ => {}
            // }
            buf.clear();
        }

        Ok(())
    }
}
