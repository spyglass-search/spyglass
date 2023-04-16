use shared::response::SimilaritySearchResult;

pub async fn similarity_search(query: &str) -> Vec<SimilaritySearchResult> {
    // search vector db, todo: if enabled/available
    let client = reqwest::Client::builder().build().unwrap();
    // todo: pull endpoint from environment / configuration
    let results: Vec<SimilaritySearchResult> = client
        .post("http://44.214.183.114:8000/search")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap_or_default();

    results
}