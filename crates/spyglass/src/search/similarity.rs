use std::env;
use shared::response::SimilaritySearchResult;

pub async fn similarity_search(query: &str) -> Vec<SimilaritySearchResult> {
    let endpoint = env::var("SIMILARITY_SEARCH_ENDPOINT")
        .unwrap_or("localhost".into());

    let port = env::var("SIMILARITY_SEARCH_PORT")
        .unwrap_or("8000".into());

    // search vector db, todo: if enabled/available
    let client = reqwest::Client::builder().build().unwrap();
    // todo: pull endpoint from environment / configuration
    let results: Vec<SimilaritySearchResult> = client
        .post(format!("http://{}:{}/search", endpoint, port))
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap_or_default();

    results
}