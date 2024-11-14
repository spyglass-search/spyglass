use std::{path::PathBuf, sync::Arc, time::Instant};

use tokenizers::Tokenizer;

use crate::{batch, load_tokenizer, Backend, CandleBackend, Embedding, ModelType, Pool};

const MAX_TOKENS: usize = 2048;

#[derive(Clone)]
pub struct EmbeddingApi {
    backend: Arc<CandleBackend>,
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

        Ok(EmbeddingApi {
            backend: Arc::new(backend),
            tokenizer,
        })
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

        let mut tokens = self
            .tokenizer
            .encode(doc_content, false)
            .map_err(|err| anyhow::format_err!("Error tokenizing {:?}", err))?;
        let token_length = tokens.len();
        if token_length > MAX_TOKENS {
            tokens.truncate(MAX_TOKENS, 1, tokenizers::TruncationDirection::Right);
        }
        let input_batch = batch(vec![tokens], [0].to_vec(), vec![]);

        let start = Instant::now();

        match self.backend.embed(input_batch) {
            Ok(embed) => {
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
            Err(error) => {
                log::error!(
                    "Embedding failed after {} tokens took {}. {:?}",
                    token_length,
                    start.elapsed().as_millis(),
                    error
                );

                Err(anyhow::format_err!("Embedding failed {:?}", error))
            }
        }
    }
}
