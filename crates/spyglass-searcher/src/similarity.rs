use reqwest::Client;
use serde::{Deserialize, Serialize};
use shared::response::SimilaritySearchResult;
use std::env;
use std::time::SystemTime;

use super::{document_to_struct, Searcher};

const EMBEDDING_ENDPOINT: &str = "SIMILARITY_SEARCH_ENDPOINT";
const EMBEDDING_PORT: &str = "SIMILARITY_SEARCH_PORT";
const DEFAULT_HOST: &str = "localhost";
const DEFAULT_PORT: &str = "8000";

#[derive(Deserialize, Serialize)]
struct SimilarityContext {
    content: String,
    document: String,
}

#[derive(Deserialize, Serialize, Debug)]
struct SimilarityContextResponse {
    context: Vec<String>,
}

pub async fn similarity_search(query: &str) -> Vec<SimilaritySearchResult> {
    let bench_start = SystemTime::now();
    let endpoint = env::var(EMBEDDING_ENDPOINT).unwrap_or(DEFAULT_HOST.into());
    let port = env::var(EMBEDDING_PORT).unwrap_or(DEFAULT_PORT.into());
    log::info!("using {}:{}", endpoint, port);
    log::info!(
        "env_check: {}ms",
        bench_start.elapsed().unwrap().as_millis()
    );

    // search vector db, todo: if enabled/available
    let bench_start = SystemTime::now();
    let client = reqwest::Client::builder().build().unwrap();
    log::info!(
        "client_build: {}ms",
        bench_start.elapsed().unwrap().as_millis()
    );

    // todo: pull endpoint from environment / configuration
    let bench_start = SystemTime::now();
    let resp = client
        .post(format!("http://{}:{}/search", endpoint, port))
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .unwrap();
    log::info!(
        "similarity_search_call: {}ms",
        bench_start.elapsed().unwrap().as_millis()
    );

    let bench_start = SystemTime::now();
    let results = resp.json().await.unwrap_or_default();
    log::info!(
        "similarity_json: {}ms",
        bench_start.elapsed().unwrap().as_millis()
    );

    results
}

pub async fn generate_similarity_context(
    searcher: &Searcher,
    query: &str,
    doc_ids: &Vec<String>,
) -> String {
    let client = Client::new();
    let mut context = Vec::new();
    let doc_sim_start = SystemTime::now();
    log::info!("Generate Similarity for {} docs", doc_ids.len());

    for doc_id in doc_ids {
        if let Some(doc) = searcher.get_by_id(doc_id) {
            if let Some(doc) = document_to_struct(&doc) {
                if let Some(doc_context) =
                    generate_similarity_context_for_doc(&client, query, &doc.content, &doc.url)
                        .await
                {
                    context.push(doc_context);
                }
            }
        }
    }
    log::info!(
        "generate_similarity_context: {}ms",
        doc_sim_start.elapsed().unwrap().as_millis()
    );
    context.join("\n")
}

pub async fn generate_similarity_context_for_doc(
    client: &Client,
    query: &str,
    content: &str,
    url: &str,
) -> Option<String> {
    let doc_sim_start = SystemTime::now();
    let endpoint = env::var(EMBEDDING_ENDPOINT).unwrap_or(DEFAULT_HOST.into());
    let port = env::var(EMBEDDING_PORT).unwrap_or(DEFAULT_PORT.into());
    log::info!("Generate Similarity Using {}:{}", endpoint, port);
    log::info!(
        "context env_check: {}ms",
        doc_sim_start.elapsed().unwrap().as_millis()
    );
    let body = SimilarityContext {
        content: String::from(query),
        document: String::from(content),
    };
    let request = client
        .post(format!("http://{}:{}/compare", endpoint, port))
        .json(&body);

    log::info!(
        "generate_similarity_context_for_doc: {}ms",
        doc_sim_start.elapsed().unwrap().as_millis()
    );

    let doc_sim_start = SystemTime::now();

    match request.send().await {
        Ok(response) => match response.json::<SimilarityContextResponse>().await {
            Ok(json) => {
                let context = json.context.join("\n");
                let doc_context = format!("URL: {}\n{}", url, context);
                log::info!(
                    "generate_similarity_context_for_doc: {}ms",
                    doc_sim_start.elapsed().unwrap().as_millis()
                );
                Some(doc_context)
            }
            Err(err) => {
                log::error!("Error processing response {:?}", err);
                None
            }
        },
        Err(err) => {
            log::error!("Error sending request {:?}", err);
            None
        }
    }
}
