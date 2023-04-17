use std::env;
use shared::response::SimilaritySearchResult;
use std::time::SystemTime;

pub async fn similarity_search(query: &str) -> Vec<SimilaritySearchResult> {
    let bench_start = SystemTime::now();
    let endpoint = env::var("SIMILARITY_SEARCH_ENDPOINT")
        .unwrap_or("localhost".into());

    let port = env::var("SIMILARITY_SEARCH_PORT")
        .unwrap_or("8000".into());
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
    let results: Vec<SimilaritySearchResult> = client
        .post(format!("http://{}:{}/search", endpoint, port))
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap_or_default();
    log::info!(
        "search_call: {}ms",
        bench_start.elapsed().unwrap().as_millis()
    );

    results
}