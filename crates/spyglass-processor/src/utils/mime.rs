pub const DOCX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
pub const DOTX: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.template";
pub const POTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.template";
pub const PPSX: &str = "application/vnd.openxmlformats-officedocument.presentationml.slideshow";
pub const PPTX: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
pub const SLDX: &str = "application/vnd.openxmlformats-officedocument.presentationml.slide";
pub const XLSX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
pub const XLTX: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.template";
pub const XLS: &str = "application/vnd.ms-excel";
pub const ODS: &str = "application/vnd.oasis.opendocument.spreadsheet";
pub const GDOC: &str = "application/vnd.google-apps.document";
pub const GSHEET: &str = "application/vnd.google-apps.spreadsheet";
pub const GSLIDES: &str = "application/vnd.google-apps.presentation";
pub const TEXT: &str = "text/plain";

use crate::utils::extensions::*;
use mime::Mime;
use strum::IntoEnumIterator;
use strum_macros::Display;

// snake_case ensures that everything is lowercase
#[derive(Clone, Debug, Display, PartialEq, Eq)]
#[strum(serialize_all = "snake_case")]
pub enum SupportedMime {
    /// Handled by our audio transcription pipeline
    Audio(Mime),
    /// Handled by our code symbol extraction pipeline
    Code(Mime),
    /// Handled by our doc/spreadsheet extraction pipeline.
    Document(Mime),
    /// Read & immediately indexed. No processing required.
    Text(Mime),
    NotSupported,
}

impl SupportedMime {
    pub fn list_all() -> Vec<Mime> {
        let mut list = Vec::new();
        list.extend(
            AudioExt::iter()
                .map(|x| x.to_string())
                .flat_map(|ext| new_mime_guess::from_ext(&ext).iter().collect::<Vec<Mime>>()),
        );
        list.extend(
            CodeExt::iter()
                .map(|x| x.to_string())
                .flat_map(|ext| new_mime_guess::from_ext(&ext).iter().collect::<Vec<Mime>>()),
        );
        list.extend(
            DocumentExt::iter()
                .map(|x| x.to_string())
                .flat_map(|ext| new_mime_guess::from_ext(&ext).iter().collect::<Vec<Mime>>()),
        );
        list.extend(
            TextExt::iter()
                .map(|x| x.to_string())
                .flat_map(|ext| new_mime_guess::from_ext(&ext).iter().collect::<Vec<Mime>>()),
        );

        list
    }

    pub fn from_mime(mime_str: &str) -> Self {
        match mime_str.parse::<Mime>() {
            Ok(mime) => {
                if AudioExt::iter().map(|x| x.to_string()).any(|extension| {
                    new_mime_guess::from_ext(&extension)
                        .iter()
                        .any(|ext_mime| ext_mime == mime)
                }) {
                    Self::Audio(mime)
                } else if TextExt::iter().map(|x| x.to_string()).any(|extension| {
                    new_mime_guess::from_ext(&extension)
                        .iter()
                        .any(|ext_mime| ext_mime == mime)
                }) {
                    Self::Text(mime)
                } else if CodeExt::iter().map(|x| x.to_string()).any(|extension| {
                    new_mime_guess::from_ext(&extension)
                        .iter()
                        .any(|ext_mime| ext_mime == mime)
                }) {
                    Self::Code(mime)
                } else if DocumentExt::iter().map(|x| x.to_string()).any(|extension| {
                    new_mime_guess::from_ext(&extension)
                        .iter()
                        .any(|ext_mime| ext_mime == mime)
                }) {
                    Self::Document(mime)
                } else {
                    Self::NotSupported
                }
            }
            Err(_) => Self::NotSupported,
        }
    }

    pub fn is_supported(mime_str: &str) -> bool {
        SupportedMime::from_mime(mime_str) != SupportedMime::NotSupported
    }
}
#[cfg(test)]
mod test {
    use crate::utils::mime::{SupportedMime, DOCX, GSLIDES, ODS, XLS, XLSX};

    #[test]
    pub fn test_document_mime_types() {
        assert_eq!(
            SupportedMime::from_mime(DOCX),
            SupportedMime::Document(DOCX.parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime(ODS),
            SupportedMime::Document(ODS.parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime(XLS),
            SupportedMime::Document(XLS.parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime(XLSX),
            SupportedMime::Document(XLSX.parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime("application/pdf"),
            SupportedMime::Document("application/pdf".parse().unwrap())
        );
    }

    #[test]
    pub fn test_code_mime_type() {
        assert_eq!(
            SupportedMime::from_mime("application/javascript"),
            SupportedMime::Code("application/javascript".parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime("text/x-rust"),
            SupportedMime::Code("text/x-rust".parse().unwrap())
        );
    }

    #[test]
    pub fn test_text_mime_type() {
        assert_eq!(
            SupportedMime::from_mime("text/plain"),
            SupportedMime::Text("text/plain".parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime("text/markdown"),
            SupportedMime::Text("text/markdown".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("text/x-markdown"),
            SupportedMime::Text("text/x-markdown".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("text/x-toml"),
            SupportedMime::Text("text/x-toml".parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime("text/x-yaml"),
            SupportedMime::Text("text/x-yaml".parse().unwrap())
        );
    }

    #[test]
    pub fn test_audio_mime_type() {
        assert_eq!(
            SupportedMime::from_mime("audio/aac"),
            SupportedMime::Audio("audio/aac".parse().unwrap())
        );

        assert_eq!(
            SupportedMime::from_mime("video/x-msvideo"),
            SupportedMime::Audio("video/x-msvideo".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("audio/flac"),
            SupportedMime::Audio("audio/flac".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("audio/m4a"),
            SupportedMime::Audio("audio/m4a".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("audio/mpeg"),
            SupportedMime::Audio("audio/mpeg".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("video/mp4"),
            SupportedMime::Audio("video/mp4".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("audio/ogg"),
            SupportedMime::Audio("audio/ogg".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("audio/wav"),
            SupportedMime::Audio("audio/wav".parse().unwrap())
        );
        assert_eq!(
            SupportedMime::from_mime("video/webm"),
            SupportedMime::Audio("video/webm".parse().unwrap())
        );
    }

    #[test]
    pub fn test_is_supported() {
        assert_eq!(SupportedMime::is_supported("application/javascript"), true);

        assert_eq!(SupportedMime::is_supported(GSLIDES), false);
    }
}
