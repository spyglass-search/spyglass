use diff::Diff;
use serde::{Deserialize, Serialize};

use super::UserSettings;
use crate::form::{FormType, SettingOpts};

pub fn audio_setting_opts(settings: &UserSettings) -> Vec<(String, SettingOpts)> {
    vec![(
        "_.audio_settings.enable_audio_transcription".into(),
        SettingOpts {
            label: "Beta: Enable Audio Indexing".into(),
            value: settings
                .audio_settings
                .enable_audio_transcription
                .to_string(),
            form_type: FormType::Bool,
            restart_required: false,
            help_text: Some(
                r#"Files with audio content (mp3s, mp4s, etc.) will be transcribed and
                the contents indexed. Enabling this will download the model
                required to do the transcription."#
                    .into(),
            ),
        },
    )]
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, Diff)]
pub struct AudioSettings {
    pub enable_audio_transcription: bool,
}
