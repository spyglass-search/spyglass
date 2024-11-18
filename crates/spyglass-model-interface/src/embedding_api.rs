use std::{path::PathBuf, sync::Arc, time::Instant};

use tokenizers::{Encoding, Tokenizer};

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
    ) -> anyhow::Result<Vec<Vec<f32>>> {
        // TODO need to properly segment the data
        let doc_content = match content_type {
            EmbeddingContentType::Document => {
                format!("search_document: {}", content.trim())
            }
            EmbeddingContentType::Query => {
                format!("search_query: {}", content.trim())
            }
        };

        let tokens = self
            .tokenizer
            .encode(doc_content, false)
            .map_err(|err| anyhow::format_err!("Error tokenizing {:?}", err))?;
        let token_length = tokens.len();
        let mut content_chunks = Vec::new();
        if token_length > MAX_TOKENS {
            let segment_count = token_length.div_ceil(MAX_TOKENS);
            let char_per_segment = content.len().div_euclid(segment_count);

            let chunks: Vec<String> = content
                .trim()
                .chars()
                .collect::<Vec<char>>()
                .chunks(char_per_segment)
                .map(|chunk| chunk.iter().collect::<String>())
                .collect();

            log::debug!(
                "Splitting text into chunks of {} chars long",
                char_per_segment
            );
            for chunk in chunks {
                let doc_content = match content_type {
                    EmbeddingContentType::Document => {
                        format!("search_document: {}", chunk)
                    }
                    EmbeddingContentType::Query => {
                        format!("search_query: {}", chunk)
                    }
                };
                let tokens = self
                    .tokenizer
                    .encode(doc_content, false)
                    .map_err(|err| anyhow::format_err!("Error tokenizing {:?}", err))?;
                log::trace!("Chunk was {} tokens long", tokens.len());
                content_chunks.push(tokens);
            }
        } else {
            content_chunks.push(tokens);
        }

        let mut embeddings = Vec::new();
        for chunk in content_chunks {
            let embedding = self.embed_tokens(chunk.to_owned())?;
            embeddings.push(embedding);
        }

        Ok(embeddings)
    }

    pub fn embed_tokens(&self, tokens: Encoding) -> anyhow::Result<Vec<f32>> {
        let token_length = tokens.len();
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
