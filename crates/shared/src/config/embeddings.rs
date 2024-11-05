use diff::Diff;
use serde::{Deserialize, Serialize};

use crate::form::{FormType, SettingOpts};

use super::UserSettings;

#[derive(Clone, Debug, Serialize, Deserialize, Diff)]
pub struct EmbeddingSettings {
    pub enable_embeddings: bool,
}

impl Default for EmbeddingSettings {
    fn default() -> Self {
        EmbeddingSettings {
            enable_embeddings: true,
        }
    }
}

#[allow(dead_code)]
pub fn embedding_setting_opts(settings: &UserSettings) -> Vec<(String, SettingOpts)> {
    vec![(
        "_.embedding_settings.enable_embeddings".into(),
        SettingOpts {
            label: "Beta: Enable Similarity Search".into(),
            value: settings.embedding_settings.enable_embeddings.to_string(),
            form_type: FormType::Bool,
            restart_required: true,
            help_text: Some(
                r#"Embeddings are generated for documents and search will check for
                   semantic similarity as well as standard search."#
                    .into(),
            ),
        },
    )]
}
