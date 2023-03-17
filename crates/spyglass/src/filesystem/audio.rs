use rubato::{InterpolationParameters, InterpolationType, Resampler, SincFixedIn, WindowFunction};
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
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

const MODEL_PATH: &str = "../../assets/models/whisper.base.en.bin";

fn resample(og: &[f32], og_rate: u32) -> Vec<f32> {
    let params = InterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: InterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler =
        SincFixedIn::<f32>::new(16_000f64 / og_rate as f64, 2.0, params, og.len(), 1).unwrap();

    let waves_in = vec![og.to_vec()];
    let mut waves_out = resampler.process(&waves_in, None).unwrap();
    waves_out.pop().unwrap_or_default()
}

// todo: handling streaming in large files
fn parse_audio_file(path: &PathBuf) -> Vec<f32> {
    let src = std::fs::File::open(path).expect("Unable open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let ext = path.extension().and_then(|x| x.to_str()).unwrap_or("mp3");

    // Create a probe hint using the file's extension. [Optional]
    let mut hint = Hint::new();
    hint.with_extension(ext);

    // Use the default options for metadata and format readers.
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    // Probe the media source.
    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .expect("unsupported format");

    // Get the instantiated format reader.
    let mut format = probed.format;

    // Find the first audio track with a known (decodeable) codec.
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .expect("no supported audio tracks");

    // Use the default options for the decoder.
    let dec_opts: DecoderOptions = Default::default();
    let sample_rate = track
        .codec_params
        .sample_rate
        .expect("No sample rate detected");

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .expect("unsupported codec");

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
            println!("{} != {}", packet.track_id(), track_id);
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

    println!(
        "decoded {} samples, buf len: {}",
        sample_count,
        samples.len()
    );

    // Do we need to resample?
    if sample_rate != 16_000 {
        println!("resampling from {} to 16000", sample_rate);
        samples = resample(&samples, sample_rate);
    }

    samples
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
fn transcibe_audio(path: PathBuf, segment_len: i32) -> Vec<Segment> {
    if !path.exists() || !path.is_file() {
        dbg!(path.exists(), path.is_file());
        panic!("expected a file");
    }

    let mut segments = Vec::new();
    let samples = parse_audio_file(&path);
    let mut ctx = WhisperContext::new(MODEL_PATH).expect("failed to open model");

    let mut params = FullParams::new(SamplingStrategy::default());
    params.set_max_len(segment_len);
    params.set_print_progress(false);

    ctx.full(params, &samples)
        .expect("failed to convert samples");
    let num_segments = ctx.full_n_segments();
    println!("Extracted {} segments", num_segments);
    for i in 0..num_segments {
        let segment = ctx.full_get_segment_text(i).expect("failed to get segment");
        let start_timestamp = ctx.full_get_segment_t0(i);
        let end_timestamp = ctx.full_get_segment_t1(i);
        segments.push(Segment::new(start_timestamp, end_timestamp, &segment));
    }

    segments
}

#[cfg(test)]
mod test {
    use super::transcibe_audio;

    #[test]
    // Use the sample from whisper.cpp as a baseline test.
    fn test_wav_transcription() {
        let expected = include_str!("../../../../fixtures/audio/jfk.txt");
        let path = "../../fixtures/audio/jfk.wav".into();
        let segments = transcibe_audio(path, 1);
        assert!(segments.len() > 0);

        let combined = segments
            .iter()
            .map(|x| x.segment.trim().to_string())
            .collect::<Vec<String>>()
            .join(" ");
        assert_eq!(combined.trim(), expected.trim());
    }

    #[test]
    // Use the sample from whisper.cpp as a baseline test.
    fn test_ogg_transcription() {
        let expected = include_str!("../../../../fixtures/audio/armstrong.txt");
        let path = "../../fixtures/audio/armstrong.ogg".into();
        let segments = transcibe_audio(path, 1);
        assert!(segments.len() > 0);
        let combined = segments
            .iter()
            .map(|x| x.segment.trim().to_string())
            .collect::<Vec<String>>()
            .join(" ");
        assert_eq!(combined.trim(), expected.trim());
    }
}
