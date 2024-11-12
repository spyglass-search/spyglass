use std::path::PathBuf;

use anyhow::anyhow;
use spyglass_model_interface::whisper::{whisper::WhisperContext, AudioMetadata, Segment};

pub struct TranscriptionResult {
    pub metadata: Option<AudioMetadata>,
    pub segments: Vec<Segment>,
}

/// Given a path to a wav file, transcribe it using our **shhhh** models.
pub fn transcribe_audio(path: PathBuf, model_path: PathBuf) -> anyhow::Result<TranscriptionResult> {
    let start = std::time::Instant::now();
    if !path.exists() || !path.is_file() {
        return Err(anyhow!("Invalid file path"));
    }

    let mut res = TranscriptionResult {
        metadata: None,
        segments: Vec::new(),
    };

    let whisper_context = WhisperContext::new(&model_path, false);

    let (segments, metadata) = whisper_context.process(&path)?;

    res.segments = segments;
    res.metadata = Some(metadata);

    log::debug!("transcribed in {} secs", start.elapsed().as_secs_f32());

    Ok(res)
}

#[cfg(test)]
mod test {
    const MODEL_PATH: &str = "../../assets/models/whisper";
    use super::transcribe_audio;

    #[test]
    fn test_wav_transcription() {
        // Use the sample from whisper.cpp as a baseline test.
        let expected = include_str!("../../../../fixtures/audio/jfk.txt");
        let path = "../../fixtures/audio/jfk.wav".into();
        let res = transcribe_audio(path, MODEL_PATH.into()).expect("Unable to transcribe");
        let segments = res.segments;
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
        let res = transcribe_audio(path, MODEL_PATH.into()).expect("Unable to transcribe");
        let segments = res.segments;
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
        let res = transcribe_audio(path, MODEL_PATH.into()).expect("Unable to transcribe");
        let segments = res.segments;
        assert!(segments.len() > 0);
        let combined = segments
            .iter()
            .map(|x| x.segment.to_string())
            .collect::<Vec<String>>()
            .join("");
        assert_eq!(combined.trim(), expected.trim());
    }
}
