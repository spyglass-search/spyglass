use std::{path::PathBuf, time::Instant};

use tokenizers::Tokenizer;

use crate::{batch, load_tokenizer, Backend, CandleBackend, Embedding, ModelType, Pool};

pub struct EmbeddingApi {
    backend: CandleBackend,
    tokenizer: Tokenizer,
}

pub enum EmbeddingContentType {
    Document,
    Query,
}

impl EmbeddingApi {
    pub fn new(model_root: PathBuf) -> anyhow::Result<Self> {
        let tokenizer = load_tokenizer(&model_root)?;
        let backend = CandleBackend::new(
            model_root,
            "float32".to_string(),
            ModelType::Embedding(Pool::Mean),
        )?;

        Ok(EmbeddingApi { backend, tokenizer })
    }

    pub fn embed(
        &self,
        content: &str,
        content_type: EmbeddingContentType,
    ) -> anyhow::Result<Vec<f32>> {
        // TODO need to properly segment the data
        let doc_content = match content_type {
            EmbeddingContentType::Document => {
                format!("search_document: {}", content)
            }
            EmbeddingContentType::Query => {
                format!("search_query: {}", content)
            }
        };

        let tokens = self
            .tokenizer
            .encode(doc_content, false)
            .map_err(|err| anyhow::format_err!("Error tokenizing {:?}", err))?;
        let token_length = tokens.len();
        let input_batch = batch(vec![tokens], [0].to_vec(), vec![]);

        let start = Instant::now();
        let embed = self.backend.embed(input_batch).unwrap();
        log::debug!(
            "Embedding {} tokens took {}",
            token_length,
            start.elapsed().as_millis()
        );
        if let Some(Embedding::Pooled(embedding)) = embed.get(&0) {
            Ok(embedding.to_owned())
        } else {
            Err(anyhow::format_err!("Unable to process embedding"))
        }
    }
}
