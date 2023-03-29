use anyhow::anyhow;
use rubato::{
    InterpolationParameters, InterpolationType, Resampler, ResamplerConstructionError, SincFixedIn,
    WindowFunction,
};
use std::path::PathBuf;
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    errors::Error,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
use whisper_rs::{convert_stereo_to_mono_audio, FullParams, SamplingStrategy, WhisperContext};

/// Resamples from the <og_rate> to the 16khz required by whisper
fn resample(og: &[f32], og_rate: u32) -> Result<Vec<f32>, ResamplerConstructionError> {
    let params = InterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: InterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler =
        SincFixedIn::<f32>::new(16_000f64 / og_rate as f64, 2.0, params, og.len(), 1)?;

    let waves_in = vec![og.to_vec()];
    let mut waves_out = resampler.process(&waves_in, None).unwrap_or_default();
    if waves_out.is_empty() {
        Ok(Vec::new())
    } else {
        Ok(waves_out.pop().unwrap_or_default())
    }
}

// todo: handling streaming in large files
fn parse_audio_file(path: &PathBuf) -> anyhow::Result<Vec<f32>> {
    let src = std::fs::File::open(path).expect("Unable open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    // Create a probe hint using the file's extension if available.
    let mut hint = Hint::new();
    if let Some(ext) = path.extension() {
        hint.with_extension(&ext.to_string_lossy());
    }

    // Use the default options for metadata and format readers.
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source.
    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    // Get the instantiated format reader.
    let mut format = probed.format;

    // Find the first audio track with a known (decodeable) codec.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL);

    let track = match track {
        Some(track) => track,
        None => return Err(anyhow!("Unable to find valid track")),
    };

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();
    let sample_rate = track.codec_params.sample_rate.unwrap_or_default();
    let channels = track.codec_params.channels.unwrap_or_default();

    if channels.count() == 0 {
        return Err(anyhow!("No audio detected!"));
    }

    if sample_rate == 0 {
        return Err(anyhow!("Invalid sample rate"));
    }

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;
    // Decode the packet into audio samples.
    let mut sample_count = 0;
    let mut samples: Vec<f32> = Vec::new();
    let mut sample_buf = None;

    // The decode loop.
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            _ => break,
        };

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            log::debug!("{} != {}", packet.track_id(), track_id);
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                if sample_buf.is_none() {
                    // Get the audio buffer specification.
                    let spec = *audio_buf.spec();
                    // Get the capacity of the decoded buffer
                    let duration = audio_buf.capacity() as u64;
                    // Create the f32 sample buffer.
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(audio_buf);
                    sample_count += buf.samples().len();
                    samples.extend(buf.samples());
                }
            }
            Err(Error::DecodeError(_)) => (),
            Err(_) => break,
        }
    }

    log::debug!("Detected {} audio channels", channels.count());
    if channels.count() > 1 {
        // convert stereo audio to mono for whisper.
        samples = convert_stereo_to_mono_audio(&samples);
    }

    log::debug!(
        "decoded {} samples, buf len: {}",
        sample_count,
        samples.len()
    );

    // Do we need to resample?
    if sample_rate != 16_000 {
        log::debug!("resampling from {} to 16000", sample_rate);
        if let Ok(new_samples) = resample(&samples, sample_rate) {
            samples = new_samples;
        } else {
            log::warn!("failed to resample, failing back to original samples");
        }
    }

    Ok(samples)
}

#[derive(Clone, Debug)]
pub struct Segment {
    pub start_timestamp: i64,
    pub end_timestamp: i64,
    pub segment: String,
}

impl Segment {
    pub fn new(start: i64, end: i64, segment: &str) -> Self {
        Self {
            start_timestamp: start,
            end_timestamp: end,
            segment: segment.to_string(),
        }
    }
}

/// Given a path to a wav file, transcribe it using our **shhhh** models.
pub fn transcibe_audio(
    path: PathBuf,
    model_path: PathBuf,
    segment_len: i32,
) -> anyhow::Result<Vec<Segment>> {
    let start = std::time::Instant::now();
    if !path.exists() || !path.is_file() {
        return Err(anyhow!("Invalid file path"));
    }

    let mut segments = Vec::new();
    match parse_audio_file(&path) {
        Ok(samples) => {
            let mut ctx = match WhisperContext::new(&model_path.to_string_lossy()) {
                Ok(ctx) => ctx,
                Err(err) => {
                    log::warn!("unable to load model: {:?}", err);
                    return Err(anyhow!("Unable to load model: {:?}", err));
                }
            };

            let mut params = FullParams::new(SamplingStrategy::default());
            params.set_max_len(segment_len);
            params.set_print_progress(false);
            params.set_token_timestamps(true);

            ctx.full(params, &samples)
                .expect("failed to convert samples");
            let num_segments = ctx.full_n_segments();
            log::debug!("Extracted {} segments", num_segments);
            for i in 0..num_segments {
                let segment = ctx.full_get_segment_text(i).expect("failed to get segment");
                let start_timestamp = ctx.full_get_segment_t0(i);
                let end_timestamp = ctx.full_get_segment_t1(i);
                segments.push(Segment::new(start_timestamp, end_timestamp, &segment));
            }
        }
        Err(err) => {
            log::warn!("Unable to parse audio file: {err}");
            return Err(anyhow!(err));
        }
    }

    log::debug!("transcribed in {} secs", start.elapsed().as_secs_f32());
    Ok(segments)
}

#[cfg(test)]
mod test {
    const MODEL_PATH: &str = "../../assets/models/whisper.base.en.bin";
    use super::transcibe_audio;

    #[test]
    fn test_wav_transcription() {
        // Use the sample from whisper.cpp as a baseline test.
        let expected = include_str!("../../../../fixtures/audio/jfk.txt");
        let path = "../../fixtures/audio/jfk.wav".into();
        let segments = transcibe_audio(path, MODEL_PATH.into(), 1).expect("Unable to transcribe");
        assert!(segments.len() > 0);

        let combined = segments
            .iter()
            .map(|x| x.segment.to_string())
            .collect::<Vec<String>>()
            .join("");
        assert_eq!(combined.trim(), expected.trim());
    }

    #[test]
    fn test_ogg_transcription() {
        let expected = include_str!("../../../../fixtures/audio/armstrong.txt");
        let path = "../../fixtures/audio/armstrong.ogg".into();
        let segments = transcibe_audio(path, MODEL_PATH.into(), 1).expect("Unable to transcribe");
        assert!(segments.len() > 0);
        let combined = segments
            .iter()
            .map(|x| x.segment.to_string())
            .collect::<Vec<String>>()
            .join("");
        assert_eq!(combined.trim(), expected.trim());
    }

    // Ignored by default since it takes a while to run
    #[ignore]
    #[test]
    fn test_mp3_transcription() {
        let expected = include_str!("../../../../fixtures/audio/count_of_monte_cristo.txt");
        let path = "../../fixtures/audio/count_of_monte_cristo.mp3".into();
        let segments = transcibe_audio(path, MODEL_PATH.into(), 1).expect("Unable to transcribe");
        assert!(segments.len() > 0);
        let combined = segments
            .iter()
            .map(|x| x.segment.to_string())
            .collect::<Vec<String>>()
            .join("");
        assert_eq!(combined.trim(), expected.trim());
    }
}
