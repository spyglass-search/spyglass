// Everything in this lib is pulled from https://github.com/huggingface/text-embeddings-inference
mod alibi;
#[cfg(feature = "cuda")]
mod compute_cap;
pub mod embedding_api;
// #[cfg(feature = "cuda")]
// mod flash_attn;
mod layers;
mod models;

#[cfg(feature = "cuda")]
use crate::compute_cap::{
    compatible_compute_cap, get_compile_compute_cap, get_runtime_compute_cap,
};
use crate::models::{
    BertConfig, BertModel, DistilBertConfig, DistilBertModel, GTEConfig, JinaBertModel,
    JinaCodeBertModel, MistralConfig, Model, NomicBertModel, NomicConfig, Qwen2Config,
};
// #[cfg(feature = "cuda")]
// use crate::models::{
//     FlashBertModel, FlashDistilBertModel, FlashGTEModel, FlashJinaBertModel,
//     FlashJinaCodeBertModel, FlashMistralModel, FlashNomicBertModel, FlashQwen2Model,
// };
use anyhow::Context;
use candle::{DType, Device};
use candle_nn::VarBuilder;
// #[cfg(feature = "clap")]
// use clap::ValueEnum;
use nohash_hasher::BuildNoHashHasher;
use nohash_hasher::IntMap;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::path::PathBuf;
use thiserror::Error;
use tokenizers::pre_tokenizers::metaspace::PrependScheme;
use tokenizers::pre_tokenizers::sequence::Sequence;
use tokenizers::{Encoding, PreTokenizerWrapper, Tokenizer};

#[derive(Debug, PartialEq, Clone)]
pub enum ModelType {
    Classifier,
    Embedding(Pool),
}

#[derive(Debug, PartialEq, Clone)]
pub enum Embedding {
    Pooled(Vec<f32>),
    All(Vec<Vec<f32>>),
}

#[derive(Debug, PartialEq, Clone)]
// #[cfg_attr(feature = "clap", derive(ValueEnum))]
pub enum Pool {
    /// Select the CLS token as embedding
    Cls,
    /// Apply Mean pooling to the model embeddings
    Mean,
    /// Apply SPLADE (Sparse Lexical and Expansion) to the model embeddings.
    /// This option is only available if the loaded model is a `ForMaskedLM` Transformer
    /// model.
    Splade,
    /// Select the last token as embedding
    LastToken,
}

impl fmt::Display for Pool {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Pool::Cls => write!(f, "cls"),
            Pool::Mean => write!(f, "mean"),
            Pool::Splade => write!(f, "splade"),
            Pool::LastToken => write!(f, "last_token"),
        }
    }
}

#[derive(Debug)]
pub struct Batch {
    pub input_ids: Vec<u32>,
    pub token_type_ids: Vec<u32>,
    pub position_ids: Vec<u32>,
    pub cumulative_seq_lengths: Vec<u32>,
    pub max_length: u32,
    pub pooled_indices: Vec<u32>,
    pub raw_indices: Vec<u32>,
}

impl Batch {
    pub fn len(&self) -> usize {
        self.cumulative_seq_lengths.len() - 1
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub type Embeddings = IntMap<usize, Embedding>;
pub type Predictions = IntMap<usize, Vec<f32>>;

#[derive(Debug, Error, Clone)]
pub enum BackendError {
    #[error("No backend found")]
    NoBackend,
    #[error("Could not start backend: {0}")]
    Start(String),
    #[error("{0}")]
    Inference(String),
    #[error("Backend is unhealthy")]
    Unhealthy,
}
pub trait Backend {
    fn health(&self) -> Result<(), BackendError>;
    fn max_batch_size(&self) -> Option<usize> {
        None
    }

    fn is_padded(&self) -> bool;

    fn embed(&self, batch: Batch) -> Result<Embeddings, BackendError>;

    fn predict(&self, batch: Batch) -> Result<Predictions, BackendError>;
}

/// This enum is needed to be able to differentiate between jina models that also use
/// the `bert` model type and valid Bert models.
/// We use the `_name_or_path` field in the config to do so. This might not be robust in the long
/// run but is still better than the other options...
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "_name_or_path")]
pub enum BertConfigWrapper {
    #[serde(rename = "jinaai/jina-bert-implementation")]
    JinaBert(BertConfig),
    #[serde(rename = "jinaai/jina-bert-v2-qk-post-norm")]
    JinaCodeBert(BertConfig),
    #[serde(untagged)]
    Bert(BertConfig),
}

#[derive(Deserialize)]
#[serde(tag = "model_type", rename_all = "kebab-case")]
#[allow(dead_code)]
enum Config {
    Bert(BertConfigWrapper),
    XlmRoberta(BertConfig),
    Camembert(BertConfig),
    Roberta(BertConfig),
    #[serde(rename(deserialize = "distilbert"))]
    DistilBert(DistilBertConfig),
    #[serde(rename(deserialize = "nomic_bert"))]
    NomicBert(NomicConfig),
    Mistral(MistralConfig),
    #[serde(rename = "new")]
    Gte(GTEConfig),
    Qwen2(Qwen2Config),
}

pub struct CandleBackend {
    device: Device,
    model: Box<dyn Model + Send + Sync>,
}

impl CandleBackend {
    pub fn new(
        model_path: PathBuf,
        dtype: String,
        model_type: ModelType,
    ) -> Result<Self, BackendError> {
        // Default files
        let default_safetensors = model_path.join("model.safetensors");
        let default_pytorch = model_path.join("pytorch_model.bin");

        // Single Files
        let model_files = if default_safetensors.exists() {
            vec![default_safetensors]
        } else if default_pytorch.exists() {
            vec![default_pytorch]
        }
        // Sharded weights
        else {
            // Get index file
            let index_file = model_path.join("model.safetensors.index.json");

            // Parse file
            let index_file_string: String = std::fs::read_to_string(&index_file)
                .map_err(|err| BackendError::Start(err.to_string()))?;
            let json: serde_json::Value = serde_json::from_str(&index_file_string)
                .map_err(|err| BackendError::Start(err.to_string()))?;

            let weight_map = match json.get("weight_map") {
                None => {
                    return Err(BackendError::Start(format!(
                        "no weight map in {index_file:?}"
                    )));
                }
                Some(serde_json::Value::Object(map)) => map,
                Some(_) => {
                    return Err(BackendError::Start(format!(
                        "weight map in {index_file:?} is not a map"
                    )));
                }
            };
            let mut safetensors_files = std::collections::HashSet::new();
            for value in weight_map.values() {
                if let Some(file) = value.as_str() {
                    safetensors_files.insert(file.to_string());
                }
            }

            // Collect paths
            safetensors_files
                .iter()
                .map(|n| model_path.join(n))
                .collect()
        };

        // Load config
        let config: String = std::fs::read_to_string(model_path.join("config.json"))
            .context("Unable to read config file")
            .map_err(|err| BackendError::Start(format!("{err:?}")))?;
        let config: Config = serde_json::from_str(&config)
            .context("Model is not supported")
            .map_err(|err| BackendError::Start(format!("{err:?}")))?;

        // Get candle device
        let device = if candle::utils::cuda_is_available() {
            #[cfg(feature = "cuda")]
            match compatible_compute_cap() {
                Ok(true) => Device::new_cuda(0),
                Ok(false) => {
                    log::error!(
                        "Runtime compute cap {} is not compatible with compile time compute cap {}",
                        get_runtime_compute_cap().unwrap(),
                        get_compile_compute_cap().unwrap()
                    );
                    Ok(Device::Cpu)
                }
                Err(err) => {
                    tracing::warn!("Could not find a compatible CUDA device on host: {err:?}");
                    tracing::warn!("Using CPU instead");
                    Ok(Device::Cpu)
                }
            }
            #[cfg(not(feature = "cuda"))]
            Ok(Device::Cpu)
        } else if candle::utils::metal_is_available() {
            Device::new_metal(0)
        } else {
            Ok(Device::Cpu)
        }
        .map_err(|err| BackendError::Start(err.to_string()))?;

        // Get candle dtype
        let dtype = if &dtype == "float32" {
            Ok(DType::F32)
        } else if &dtype == "float16" {
            Ok(DType::F16)
        } else {
            Err(BackendError::Start(format!(
                "DType {dtype} is not supported"
            )))
        }?;

        let vb = if model_files.len() == 1 && model_files[0].extension().unwrap() == "bin" {
            VarBuilder::from_pth(&model_files[0], dtype, &device)
        } else {
            unsafe { VarBuilder::from_mmaped_safetensors(&model_files, dtype, &device) }
        }
        .s()?;

        let model: Result<Box<dyn Model + Send + Sync>, BackendError> = match (config, &device) {
            (Config::Bert(config), _) => match config {
                BertConfigWrapper::JinaBert(config) => {
                    tracing::info!("Starting JinaBert model on {:?}", device);
                    Ok(Box::new(JinaBertModel::load(vb, &config, model_type).s()?))
                }
                BertConfigWrapper::JinaCodeBert(config) => {
                    tracing::info!("Starting JinaCodeBert model on {:?}", device);
                    Ok(Box::new(
                        JinaCodeBertModel::load(vb, &config, model_type).s()?,
                    ))
                }
                BertConfigWrapper::Bert(config) => {
                    tracing::info!("Starting Bert model on {:?}", device);
                    Ok(Box::new(BertModel::load(vb, &config, model_type).s()?))
                }
            },
            (
                Config::XlmRoberta(config) | Config::Camembert(config) | Config::Roberta(config),
                _,
            ) => {
                tracing::info!("Starting Bert model on {:?}", device);
                Ok(Box::new(
                    BertModel::load_roberta(vb, &config, model_type).s()?,
                ))
            }
            (Config::DistilBert(config), _) => {
                tracing::info!("Starting DistilBert model on {:?}", device);
                Ok(Box::new(
                    DistilBertModel::load(vb, &config, model_type).s()?,
                ))
            }
            (Config::NomicBert(config), _) => {
                tracing::info!("Starting NomicBert model on {:?}", device);
                Ok(Box::new(NomicBertModel::load(vb, &config, model_type).s()?))
            }
            (Config::Mistral(_), _) => Err(BackendError::Start(
                "Mistral is only supported on Cuda devices in fp16 with flash attention enabled"
                    .to_string(),
            )),
            (Config::Gte(_), _) => Err(BackendError::Start(
                "GTE is only supported on Cuda devices in fp16 with flash attention enabled"
                    .to_string(),
            )),
            (Config::Qwen2(_), _) => Err(BackendError::Start(
                "Qwen2 is only supported on Cuda devices in fp16 with flash attention enabled"
                    .to_string(),
            )),
        };

        Ok(Self {
            device,
            model: model?,
        })
    }
}

impl Backend for CandleBackend {
    fn max_batch_size(&self) -> Option<usize> {
        // Limit max batch size to 4 on CPU
        if matches!(self.device, Device::Cpu) {
            return Some(4);
        }
        None
    }

    fn health(&self) -> Result<(), BackendError> {
        Ok(())
    }

    fn is_padded(&self) -> bool {
        self.model.is_padded()
    }

    fn embed(&self, batch: Batch) -> Result<Embeddings, BackendError> {
        let batch_size = batch.len();
        let pooled_indices = batch.pooled_indices.clone();
        let raw_indices = batch.raw_indices.clone();

        // Used for indexing in the raw_embeddings tensor
        let input_lengths: Vec<usize> = (0..batch.len())
            .map(|i| {
                (batch.cumulative_seq_lengths[i + 1] - batch.cumulative_seq_lengths[i]) as usize
            })
            .collect();

        // Run forward
        let (pooled_embeddings, raw_embeddings) = self.model.embed(batch).e()?;

        // Device => Host data transfer
        let pooled_embeddings = match pooled_embeddings {
            None => vec![],
            Some(pooled_embeddings) => pooled_embeddings.to_dtype(DType::F32).e()?.to_vec2().e()?,
        };

        // This transfer is expensive...
        let raw_embeddings = match raw_embeddings {
            None => vec![],
            Some(raw_embeddings) => raw_embeddings.to_dtype(DType::F32).e()?.to_vec2().e()?,
        };

        let mut embeddings =
            HashMap::with_capacity_and_hasher(batch_size, BuildNoHashHasher::default());
        for (i, e) in pooled_indices.into_iter().zip(pooled_embeddings) {
            embeddings.insert(i as usize, Embedding::Pooled(e));
        }

        let mut cumulative_length = 0;
        for i in raw_indices.into_iter() {
            let length = input_lengths[i as usize];
            let e = raw_embeddings[cumulative_length..cumulative_length + length].to_vec();
            embeddings.insert(i as usize, Embedding::All(e));
            cumulative_length += length;
        }

        Ok(embeddings)
    }

    fn predict(&self, batch: Batch) -> Result<Predictions, BackendError> {
        let batch_size = batch.len();

        let results = self.model.predict(batch).e()?;
        let results = results.to_dtype(DType::F32).e()?.to_vec2().e()?;

        let mut predictions =
            HashMap::with_capacity_and_hasher(batch_size, BuildNoHashHasher::default());
        for (i, r) in results.into_iter().enumerate() {
            predictions.insert(i, r);
        }

        Ok(predictions)
    }
}

pub trait WrapErr<O> {
    fn s(self) -> Result<O, BackendError>;
    fn e(self) -> Result<O, BackendError>;
}

impl<O> WrapErr<O> for Result<O, candle::Error> {
    fn s(self) -> Result<O, BackendError> {
        self.map_err(|e| BackendError::Start(e.to_string()))
    }
    fn e(self) -> Result<O, BackendError> {
        self.map_err(|e| BackendError::Inference(e.to_string()))
    }
}

pub fn load_tokenizer(model_root: &Path) -> anyhow::Result<Tokenizer> {
    // Load tokenizer
    let tokenizer_path = model_root.join("tokenizer.json");
    let mut tokenizer = Tokenizer::from_file(tokenizer_path).expect("tokenizer.json not found");
    // See https://github.com/huggingface/tokenizers/pull/1357
    if let Some(pre_tokenizer) = tokenizer.get_pre_tokenizer() {
        if let PreTokenizerWrapper::Metaspace(m) = pre_tokenizer {
            // We are forced to clone since `Tokenizer` does not have a `get_mut` for `pre_tokenizer`
            let mut m = m.clone();
            m.set_prepend_scheme(PrependScheme::First);
            tokenizer.with_pre_tokenizer(Some(PreTokenizerWrapper::Metaspace(m)));
        } else if let PreTokenizerWrapper::Sequence(s) = pre_tokenizer {
            let pre_tokenizers = s.get_pre_tokenizers();
            // Check if we have a Metaspace pre tokenizer in the sequence
            let has_metaspace = pre_tokenizers
                .iter()
                .any(|t| matches!(t, PreTokenizerWrapper::Metaspace(_)));

            if has_metaspace {
                let mut new_pre_tokenizers = Vec::with_capacity(s.get_pre_tokenizers().len());

                for pre_tokenizer in pre_tokenizers {
                    if let PreTokenizerWrapper::WhitespaceSplit(_) = pre_tokenizer {
                        // Remove WhitespaceSplit
                        // This will be done by the Metaspace pre tokenizer
                        continue;
                    }

                    let mut pre_tokenizer = pre_tokenizer.clone();

                    if let PreTokenizerWrapper::Metaspace(ref mut m) = pre_tokenizer {
                        m.set_prepend_scheme(PrependScheme::First);
                    }
                    new_pre_tokenizers.push(pre_tokenizer);
                }
                tokenizer.with_pre_tokenizer(Some(PreTokenizerWrapper::Sequence(Sequence::new(
                    new_pre_tokenizers,
                ))));
            }
        }
    }

    tokenizer.with_padding(None);
    Ok(tokenizer)
}

pub fn batch(encodings: Vec<Encoding>, pooled_indices: Vec<u32>, raw_indices: Vec<u32>) -> Batch {
    let mut input_ids = Vec::new();
    let mut token_type_ids = Vec::new();
    let mut position_ids = Vec::new();
    let mut cumulative_seq_lengths = Vec::with_capacity(encodings.len() + 1);
    cumulative_seq_lengths.push(0);

    let mut max_length = 0;
    let mut cumulative_length = 0;

    for encoding in encodings.iter() {
        let encoding_length = encoding.len() as u32;
        input_ids.extend(encoding.get_ids().to_vec());
        token_type_ids.extend(encoding.get_type_ids().to_vec());
        position_ids.extend(0..encoding_length);
        cumulative_length += encoding_length;
        cumulative_seq_lengths.push(cumulative_length);
        max_length = std::cmp::max(max_length, encoding_length);
    }

    Batch {
        input_ids,
        token_type_ids,
        position_ids,
        cumulative_seq_lengths,
        max_length,
        pooled_indices,
        raw_indices,
    }
}
