use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct CollectorConfiguration {}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct ParserConfiguration {}

// Pipeline user configuration
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct PipelineConfiguration {
    pub kind: String,
    #[serde(default)]
    pub collector: Option<CollectorConfiguration>,
    #[serde(default)]
    pub parser: Option<ParserConfiguration>,
}
