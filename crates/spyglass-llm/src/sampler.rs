use anyhow::Result;
use candle::Tensor;
use candle_transformers::generation::{LogitsProcessor, Sampling};

use crate::model::LLMModel;

pub struct Sampler {
    model: LLMModel,
    processor: LogitsProcessor,
    num_sampled: usize,
    last_token: Option<u32>,
}

impl Sampler {
    pub fn new(model: &LLMModel) -> Self {
        Self {
            model: model.clone(),
            processor: LogitsProcessor::from_sampling(0, Sampling::ArgMax),
            num_sampled: 0,
            last_token: None,
        }
    }

    fn sample(&mut self, tokens: &[u32], index: usize) -> Result<u32> {
        let input = Tensor::new(tokens, self.model.device())?.unsqueeze(0)?;
        let logits = self.model.weights.forward(&input, index)?;
        let logits = logits.squeeze(0)?;
        let next_token = self.processor.sample(&logits)?;
        Ok(next_token)
    }

    pub fn load_prompt(&mut self, prompt: &str) -> Result<u32> {
        let prompt_tokens = self.model.encode(&prompt)?;
        let next_token: u32 = self.sample(&prompt_tokens, 0)?;
        self.last_token = Some(next_token);
        self.num_sampled += prompt_tokens.len();

        Ok(next_token)
    }

    pub fn next(&mut self) -> Result<u32> {
        let slice = if let Some(token) = self.last_token {
            vec![token]
        } else {
            Vec::new()
        };

        let next_token = self.sample(&slice, self.num_sampled)?;
        self.num_sampled += 1;
        self.last_token.replace(next_token);

        Ok(next_token)
    }

    pub fn is_done(&self) -> bool {
        self.last_token.map(|x| x == self.model.eos_token).unwrap_or_default()
    }
}
