use std::str::FromStr;

use strum_macros::{Display, EnumString};

// snake_case ensures that everything is lowercase
#[derive(Clone, Debug, Display, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum SupportedExt {
    /// Handle by our audio transcription pipeline
    Audio(AudioExt),
    /// Handled by our code symbol extraction pipeline
    Code(CodeExt),
    /// Read & immediately indexed. No processing required.
    Text(TextExt),
    NotSupported,
}

impl SupportedExt {
    pub fn from_ext(ext: &str) -> Self {
        let ext = ext.to_lowercase();
        if let Ok(ext) = AudioExt::from_str(&ext) {
            Self::Audio(ext)
        } else if let Ok(ext) = CodeExt::from_str(&ext) {
            Self::Code(ext)
        } else {
            Self::NotSupported
        }
    }
}

#[derive(Clone, Debug, Display, EnumString, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum AudioExt {
    M4a,
    Wav,
}

#[derive(Clone, Debug, Display, EnumString, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum CodeExt {
    C,
    Cpp,
    Js,
    Rs,
    Ts,
}

#[derive(Clone, Debug, Display, EnumString, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum TextExt {
    Cfg,
    Csv,
    Json,
    Md,
    Rtf,
    Toml,
    Txt,
    Xml,
    Yaml,
    Yml,
}

#[cfg(test)]
mod test {
    use super::SupportedExt;
    use crate::filesystem::extensions::AudioExt;

    #[test]
    fn test_extension_to_enum() {
        let path = std::path::Path::new("/tmp/fake_path.wav");
        let ext = path.extension().unwrap().to_string_lossy();

        let ext = SupportedExt::from_ext(&ext);
        assert_eq!(ext, SupportedExt::Audio(AudioExt::Wav));

        let path = std::path::Path::new("/tmp/fake_path.pth");
        let ext = path.extension().unwrap().to_string_lossy();

        let ext = SupportedExt::from_ext(&ext);
        assert_eq!(ext, SupportedExt::NotSupported);
    }

    #[test]
    fn test_extension_to_enum_variations() {
        let path = std::path::Path::new("/tmp/fake_path.WAV");
        let ext = path.extension().unwrap().to_string_lossy();

        let ext = SupportedExt::from_ext(&ext);
        assert_eq!(ext, SupportedExt::Audio(AudioExt::Wav));
    }
}
