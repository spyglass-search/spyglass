use anyhow::Result;
use std::path::PathBuf;

use candle::{backend::BackendDevice, quantized::gguf_file, Device, MetalDevice};
use candle_transformers::models::quantized_llama::ModelWeights;
use tokenizers::Tokenizer;

use crate::{sampler::Sampler, token_output_stream::TokenOutputStream};

fn format_size(size_in_bytes: usize) -> String {
    if size_in_bytes < 1_000 {
        format!("{}B", size_in_bytes)
    } else if size_in_bytes < 1_000_000 {
        format!("{:.2}KB", size_in_bytes as f64 / 1e3)
    } else if size_in_bytes < 1_000_000_000 {
        format!("{:.2}MB", size_in_bytes as f64 / 1e6)
    } else {
        format!("{:.2}GB", size_in_bytes as f64 / 1e9)
    }
}

#[derive(Clone)]
pub struct LLMModel {
    device: Device,
    pub weights: ModelWeights,
    pub stream: TokenOutputStream,
    pub eos_token: u32,
}

impl LLMModel {
    pub fn new(gguf_path: PathBuf) -> Result<Self> {
        let mut timer = std::time::Instant::now();

        let device = if cfg!(target_os = "macos") {
            match MetalDevice::new(0) {
                Ok(dev) => candle::Device::Metal(dev),
                Err(err) => {
                    log::warn!("Using CPU fallback. Unable to create MetalDevice: {err}");
                    candle::Device::Cpu
                }
            }
        } else {
            candle::Device::Cpu
        };

        let mut file = std::fs::File::open(gguf_path)?;
        let model = gguf_file::Content::read(&mut file)?;
        dbg!(model.metadata.keys());

        let mut total_size_in_bytes = 0;
        for (_, tensor) in model.tensor_infos.iter() {
            let elem_count = tensor.shape.elem_count();
            total_size_in_bytes +=
                elem_count * tensor.ggml_dtype.type_size() / tensor.ggml_dtype.block_size();
        }

        log::info!(
            "loaded {:?} tensors ({}) in {:.2}s",
            model.tensor_infos.len(),
            &format_size(total_size_in_bytes),
            timer.elapsed().as_secs_f32(),
        );

        // todo: load tokenizer from gguf file itself.
        log::info!("loading tokenizer & weights");
        timer = std::time::Instant::now();
        let tokenizer = Tokenizer::from_file("assets/models/llm/llama3/tokenizer.json")
            .map_err(anyhow::Error::msg)?;

        let weights = ModelWeights::from_gguf(model, &mut file, &device)?;
        let tos = TokenOutputStream::new(tokenizer.clone());
        log::info!("total load took: {:.3}s", timer.elapsed().as_secs_f32());

        let eos_token = "<|eot_id|>";
        let eos_token = *tos.tokenizer().get_vocab(true).get(eos_token).unwrap();

        Ok(Self {
            device,
            weights,
            eos_token,
            stream: tos,
        })
    }

    pub fn device(&self) -> &Device {
        &self.device
    }

    pub fn encode(&self, content: &str) -> Result<Vec<u32>> {
        let tokens = self
            .stream
            .tokenizer()
            .encode(content, true)
            .map_err(anyhow::Error::msg)?;
        Ok(tokens.get_ids().to_vec())
    }

    pub fn sampler(&self) -> Sampler {
        Sampler::new(self)
    }
}
