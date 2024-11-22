
use spyglass_llm::run_model;

#[tokio::main]
pub async fn main() -> Result<(), anyhow::Error> {
    // Default to info logging if nothing is set.
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }

    pretty_env_logger::init();

    run_model("assets/models/llm/llama3/Llama-3.2-3B-Instruct.Q5_K_M.gguf".into()).await
}
