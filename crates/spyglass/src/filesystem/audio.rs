use std::path::PathBuf;
use symphonia::core::{
    audio::{AudioBuffer, AudioBufferRef},
    codecs::{DecoderOptions, CODEC_TYPE_NULL},
    formats::FormatOptions,
    io::MediaSourceStream,
    meta::MetadataOptions,
    probe::Hint,
};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

const MODEL_PATH: &str = "assets/models/whisper.base.en.bin";

fn parse_audio_file(path: &PathBuf) -> Vec<i16> {
    let src = std::fs::File::open(path).expect("Unable open media");
    let mss = MediaSourceStream::new(Box::new(src), Default::default());

    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or_else(|| "mp3");

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

    // Create a decoder for the track.
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .expect("unsupported codec");

    // Store the track identifier, it will be used to filter packets.
    let track_id = track.id;
    // The decode loop.
    let mut samples = Vec::new();
    loop {
        // Get the next packet from the media format.
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            _ => return Vec::new(),
        };

        // If the packet does not belong to the selected track, skip over it.
        if packet.track_id() != track_id {
            continue;
        }

        // Decode the packet into audio samples.
        match &decoder.decode(&packet) {
            Ok(decoded) => {
                // Consume the decoded audio samples (see below).
                match decoded {
                    AudioBufferRef::S16(sample) => samples.push(sample.in),
                    _ => continue,
                }
            }
            Err(_) => return Vec::new(),
        }
    }

    samples
}

/// Given a path to a wav file, transcribe it using our **shhhh** models.
fn transcibe_wav(path: PathBuf) -> String {
    if !path.exists() || !path.is_file() {
        panic!("expected a file");
    }

    let original_samples = parse_audio_file(&path);
    let samples = whisper_rs::convert_integer_to_float_audio(&original_samples);

    todo!()
}

#[cfg(test)]
mod test {
    #[test]
    fn test_basic_transcription() {}
}
