use std::path::PathBuf;

use anyhow::anyhow;
use rubato::{
    Resampler, ResamplerConstructionError, SincFixedIn, SincInterpolationParameters,
    SincInterpolationType, WindowFunction,
};
use symphonia::core::{
    audio::SampleBuffer,
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    errors::Error,
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::{MetadataOptions, StandardTagKey},
    probe::Hint,
};

pub mod decoder;
pub mod multilingual;
pub mod whisper;

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

#[derive(Default)]
pub struct AudioMetadata {
    pub album: Option<String>,
    pub artist: Option<String>,
    pub title: Option<String>,
}

pub struct AudioFile {
    pub metadata: AudioMetadata,
    pub samples: Vec<f32>,
}

/// Resamples from the <og_rate> to the 16khz required by whisper
fn resample(og: &[f32], og_rate: u32) -> Result<Vec<f32>, ResamplerConstructionError> {
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
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
fn parse_audio_file(path: &PathBuf) -> anyhow::Result<AudioFile> {
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
    let mut probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

    // Grab any metadata about the file we can use to for the doc (title / author)
    let container_metadata = probed.format.metadata();
    let source_metadata = probed.metadata.get();
    let tags = if let Some(metadata) = container_metadata.current() {
        metadata.tags()
    } else if let Some(metadata_rev) = source_metadata.as_ref().and_then(|m| m.current()) {
        metadata_rev.tags()
    } else {
        &[]
    };

    log::debug!("found {} metadata tags", tags.len());
    let mut audio_meta = AudioMetadata::default();
    for tag in tags.iter().filter(|x| x.is_known()) {
        if let Some(key) = tag.std_key {
            match key {
                StandardTagKey::Album => {
                    audio_meta.album = Some(tag.value.to_string());
                }
                StandardTagKey::AlbumArtist | StandardTagKey::Artist => {
                    audio_meta.artist = Some(tag.value.to_string());
                }
                StandardTagKey::TrackTitle => {
                    audio_meta.title = Some(tag.value.to_string());
                }
                _ => {}
            }
        }
    }

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
    // NOTE: Having 0 channels doesn't necessarily mean there's no audio.
    let channels = track.codec_params.channels.unwrap_or_default();

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
                    // Get t he capacity of the decoded buffer
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
        if samples.len() & 1 == 0 {
            samples = samples
                .chunks_exact(2)
                .map(|x| (x[0] + x[1]) / 2.0)
                .collect();
        }
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

    Ok(AudioFile {
        metadata: audio_meta,
        samples,
    })
}
