use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

pub mod config;
pub mod event;
pub mod regex;
pub mod request;
pub mod response;
pub mod rpc;

#[derive(Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize)]
pub enum FormType {
    List,
    Text,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SettingOpts {
    pub label: String,
    pub value: String,
    pub form_type: FormType,
    pub help_text: Option<String>,
}
