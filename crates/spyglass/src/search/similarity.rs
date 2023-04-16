use shared::response::VectorSearchResult;

pub async fn similarity_search(query: &str) -> Vec<VectorSearchResult> {
    // search vector db, todo: if enabled/available
    let client = reqwest::Client::builder().build().unwrap();
    // todo: pull endpoint from environment / configuration
    let vector_results: Vec<VectorSearchResult> = client
        .post("http://44.214.183.114:8000/search")
        .json(&serde_json::json!({ "query": query }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap_or_default();

    vector_results
}