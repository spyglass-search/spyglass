use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumString};

#[derive(Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize, Eq)]
pub enum FormType {
    Path,
    PathList,
    Text,
}

impl FormType {
    pub fn validate(&self, value: &str) -> Result<String, String> {
        let value = value.trim();
        match self {
            FormType::Path => {
                // Escape backslashes
                let value = value.replace('\\', "\\\\");
                Ok(value)
            }
            FormType::PathList => {
                // Escape backslashes
                let value = value.replace('\\', "\\\\");
                // Validate the value by attempting to deserialize
                match serde_json::from_str::<Vec<String>>(&value) {
                    Ok(parsed) => {
                        Ok(serde_json::to_string::<Vec<String>>(&parsed).expect("Invalid list"))
                    }
                    Err(e) => Err(e.to_string()),
                }
            }
            FormType::Text => {
                if value.is_empty() {
                    return Err("Value cannot be empty".into());
                }

                Ok(value.into())
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SettingOpts {
    pub label: String,
    pub value: String,
    pub form_type: FormType,
    pub help_text: Option<String>,
}
