use serde::{Deserialize, Serialize};
use std::path::Path;
use strum_macros::{Display, EnumString};

use crate::keyboard::KeyCode;
use crate::{accelerator, MAC_OS};

#[derive(Clone, Debug, Display, EnumString, PartialEq, Serialize, Deserialize, Eq)]
pub enum FormType {
    Bool,
    /// Assumes non-negative number.
    Number,
    Path,
    PathList,
    StringList,
    Text,
    KeyBinding,
}

impl FormType {
    pub fn validate(&self, value: &str) -> Result<String, String> {
        let value = value.trim();
        match self {
            FormType::Bool => match serde_json::from_str::<bool>(value) {
                Ok(_) => Ok(value.to_string()),
                Err(e) => Err(e.to_string()),
            },
            FormType::Number => match serde_json::from_str::<u64>(value) {
                Ok(_) => Ok(value.to_string()),
                Err(e) => Err(e.to_string()),
            },
            FormType::StringList => {
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
            FormType::Path => {
                // Escape backslashes
                let value = value.to_owned();
                let existence_check = Path::new(&value);
                if existence_check.exists() {
                    Ok(value)
                } else {
                    Err(format!("Path \"{value}\" is not accessible/does not exist"))
                }
            }
            FormType::PathList => {
                // Escape backslashes
                let value = value.replace('\\', "\\\\");
                // Validate the value by attempting to deserialize
                match serde_json::from_str::<Vec<String>>(&value) {
                    Ok(parsed) => {
                        for path in parsed.iter() {
                            let check = Path::new(&path);
                            if !check.exists() {
                                return Err(format!(
                                    "Path \"{path}\" is not accessible/does not exist"
                                ));
                            }
                        }

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
            FormType::KeyBinding => {
                if value.is_empty() {
                    return Err("Value cannot be empty".into());
                }

                match accelerator::parse_accelerator(value, MAC_OS) {
                    Ok(acc) => {
                        if !acc.mods.alt_key() && !acc.mods.control_key() && !acc.mods.super_key() {
                            return Err("Global key binding must have at least one modifier key (ALT, CMD, CTRL)".into());
                        }

                        if let KeyCode::Unidentified(_) = acc.key {
                            return Err("Invalid key code binding".into());
                        }
                    }
                    Err(error) => {
                        return Err(format!(
                            "Value does not represent a valid key binding {:?}",
                            error
                        ));
                    }
                }

                Ok(value.to_owned())
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
    #[serde(default)]
    pub restart_required: bool,
}
