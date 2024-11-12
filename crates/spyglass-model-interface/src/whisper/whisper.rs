use std::path::{Path, PathBuf};

use candle::{
    utils::{cuda_is_available, metal_is_available},
    Device, Tensor,
};

use anyhow::Error;
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self, audio, Config};
use tokenizers::Tokenizer;

use crate::whisper::{
    decoder::{Decoder, Task},
    multilingual,
};

use super::{AudioMetadata, Segment};

pub enum Model {
    Normal(candle_transformers::models::whisper::model::Whisper),
    Quantized(candle_transformers::models::whisper::quantized_model::Whisper),
}

// Maybe we should use some traits rather than doing the dispatch for all these.
impl Model {
    pub fn config(&self) -> &Config {
        match self {
            Self::Normal(m) => &m.config,
            Self::Quantized(m) => &m.config,
        }
    }

    pub fn encoder_forward(&mut self, x: &Tensor, flush: bool) -> candle::Result<Tensor> {
        match self {
            Self::Normal(m) => m.encoder.forward(x, flush),
            Self::Quantized(m) => m.encoder.forward(x, flush),
        }
    }

    pub fn decoder_forward(
        &mut self,
        x: &Tensor,
        xa: &Tensor,
        flush: bool,
    ) -> candle::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.forward(x, xa, flush),
            Self::Quantized(m) => m.decoder.forward(x, xa, flush),
        }
    }

    pub fn decoder_final_linear(&self, x: &Tensor) -> candle::Result<Tensor> {
        match self {
            Self::Normal(m) => m.decoder.final_linear(x),
            Self::Quantized(m) => m.decoder.final_linear(x),
        }
    }
}

pub struct WhisperContext {
    config_path: PathBuf,
    model_path: PathBuf,
    tokenizer_path: PathBuf,
    multilingual: bool,
}

impl WhisperContext {
    pub fn new(model_root: &Path, multilingual: bool) -> Self {
        let config_path = model_root.join("config.json");
        let model_path = model_root.join("model.safetensors");
        let tokenizer_path = model_root.join("tokenizer.json");

        WhisperContext {
            config_path,
            model_path,
            tokenizer_path,
            multilingual,
        }
    }

    pub fn process(&self, input: &PathBuf) -> anyhow::Result<(Vec<Segment>, AudioMetadata)> {
        let device = get_device()?;

        let config: Config =
            serde_json::from_str(&std::fs::read_to_string(self.config_path.clone())?)?;
        let tokenizer = Tokenizer::from_file(self.tokenizer_path.clone()).map_err(Error::msg)?;

        let mel_bytes = match config.num_mel_bins {
            80 => include_bytes!("melfilters.bytes").as_slice(),
            128 => include_bytes!("melfilters128.bytes").as_slice(),
            nmel => anyhow::bail!("unexpected num_mel_bins {nmel}"),
        };

        let mut mel_filters = vec![0f32; mel_bytes.len() / 4];
        <byteorder::LittleEndian as byteorder::ByteOrder>::read_f32_into(
            mel_bytes,
            &mut mel_filters,
        );

        let audio_file = super::parse_audio_file(&input)?;
        let metadata = audio_file.metadata;

        log::debug!("pcm data loaded {}", audio_file.samples.len());

        let mel = audio::pcm_to_mel(&config, &audio_file.samples, &mel_filters);
        let mel_len = mel.len();
        let mel = Tensor::from_vec(
            mel,
            (1, config.num_mel_bins, mel_len / config.num_mel_bins),
            &device,
        )?;
        log::debug!("loaded mel: {:?}", mel.dims());

        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(
                &[self.model_path.clone()],
                whisper::DTYPE,
                &device,
            )?
        };
        let mut model =
            Model::Normal(whisper::model::Whisper::load(&vb, config).map_err(Error::msg)?);

        let language_token = if self.multilingual {
            multilingual::detect_language(&mut model, &tokenizer, &mel).ok()
        } else {
            None
        };
        let mut dc = Decoder::new(
            model,
            tokenizer,
            rand::random(),
            &device,
            language_token,
            Some(Task::Transcribe),
            false,
            false,
        )?;

        dc.run(&mel).map(|segments| {
            (
                segments
                    .iter()
                    .map(|segment| Segment {
                        start_timestamp: segment.start as i64,
                        end_timestamp: (segment.start + segment.duration) as i64,
                        segment: segment.dr.text.clone(),
                    })
                    .collect::<Vec<Segment>>(),
                metadata,
            )
        })
    }
}

pub fn get_device() -> anyhow::Result<Device> {
    if cuda_is_available() {
        Ok(Device::new_cuda(0)?)
    } else if metal_is_available() {
        Ok(Device::new_metal(0)?)
    } else {
        Ok(Device::Cpu)
    }
}
