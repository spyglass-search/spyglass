use std::collections::HashMap;
use std::path::PathBuf;

use shared::config::FileSystemSettings;
use shared::config::UserActionSettings;
use tauri::Manager;
use tauri::State;

use shared::config::{Config, Limit, UserSettings};
use shared::form::SettingOpts;

#[tauri::command]
pub async fn save_user_settings(
    window: tauri::Window,
    config: State<'_, Config>,
    settings: HashMap<String, String>,
    restart: bool,
) -> Result<(), HashMap<String, String>> {
    let mut current_settings =
        Config::load_user_settings().unwrap_or_else(|_| config.user_settings.clone());
    let orig_settings = current_settings.clone();

    let config_list: Vec<(String, SettingOpts)> = config.user_settings.clone().into();
    let setting_configs: HashMap<String, SettingOpts> = config_list.into_iter().collect();
    let mut errors: HashMap<String, String> = HashMap::new();

    let mut fields_updated: usize = 0;
    // Loop through each updated settings value sent from the front-end and
    // validate the values.
    for (key, value) in settings.iter() {
        // Update spyglass or plugin settings?
        if let Some((parent, field)) = key.split_once('.') {
            if parent == "_" {
                // Hacky way to update user settings directly.

                if let Some(opt) = setting_configs.get(key) {
                    match opt.form_type.validate(value) {
                        Ok(val) => {
                            fields_updated += 1;
                            match field {
                                "data_directory" => {
                                    current_settings.data_directory = PathBuf::from(val);
                                }
                                "shortcut" => {
                                    current_settings.shortcut = val;
                                }
                                "disable_autolaunch" => {
                                    current_settings.disable_autolaunch =
                                        serde_json::from_str(value).unwrap_or_default();
                                }
                                "close_search_bar" => {
                                    current_settings.close_search_bar =
                                        serde_json::from_str(value).unwrap_or_default();
                                }
                                "disable_telemetry" => {
                                    current_settings.disable_telemetry =
                                        serde_json::from_str(value).unwrap_or_default();
                                }
                                "inflight_crawl_limit" => {
                                    let limit: u32 = serde_json::from_str(value).unwrap_or(10);
                                    current_settings.inflight_crawl_limit = Limit::Finite(limit);
                                }
                                "inflight_domain_limit" => {
                                    let limit: u32 = serde_json::from_str(value).unwrap_or(2);
                                    current_settings.inflight_domain_limit = Limit::Finite(limit);
                                }
                                "port" => {
                                    current_settings.port = serde_json::from_str(value)
                                        .unwrap_or_else(|_| UserSettings::default_port());
                                }
                                "filesystem_settings.watched_paths" => {
                                    current_settings.filesystem_settings.watched_paths =
                                        serde_json::from_str(value).unwrap_or_else(|_| {
                                            FileSystemSettings::default().watched_paths
                                        })
                                }
                                "filesystem_settings.supported_extensions" => {
                                    current_settings.filesystem_settings.supported_extensions =
                                        serde_json::from_str(value).unwrap_or_else(|_| {
                                            FileSystemSettings::default().supported_extensions
                                        })
                                }
                                "filesystem_settings.enable_filesystem_scanning" => {
                                    current_settings
                                        .filesystem_settings
                                        .enable_filesystem_scanning =
                                        serde_json::from_str(value).unwrap_or_default()
                                }
                                "audio_settings.enable_audio_transcription" => {
                                    current_settings.audio_settings.enable_audio_transcription =
                                        serde_json::from_str(value).unwrap_or_default()
                                }
                                "embedding_settings.enable_embeddings" => {
                                    current_settings.embedding_settings.enable_embeddings =
                                        serde_json::from_str(value).unwrap_or_default()
                                }
                                _ => {}
                            }
                        }
                        Err(err) => {
                            errors.insert(key.to_string(), err);
                        }
                    }
                }
            }
        }
    }

    // Only save settings if everything is valid.
    if errors.is_empty() && fields_updated > 0 {
        match crate::cmd::update_user_settings(window.clone(), &current_settings).await {
            Ok(updates) => {
                if restart {
                    let app = window.app_handle();
                    app.restart();
                } else {
                    crate::configuration_updated(window, orig_settings, updates);
                }
                Ok(())
            }
            Err(error) => {
                let mut map = HashMap::new();
                map.insert(String::from("all"), error);
                Err(map)
            }
        }
    } else {
        Err(errors)
    }
}

#[tauri::command]
pub async fn load_action_settings(
    _: tauri::Window,
    _config: State<'_, Config>,
) -> Result<UserActionSettings, String> {
    let settings = Config::load_user_settings().expect("unable to read user settings");
    let user_action_settings = settings.user_action_settings;
    Ok(user_action_settings)
}

#[tauri::command]
pub async fn load_user_settings(
    window: tauri::Window,
    _config: State<'_, Config>,
) -> Result<Vec<(String, SettingOpts)>, String> {
    let current_settings = crate::cmd::user_settings(window)
        .await
        .expect("Unable to read user settings");

    let list: Vec<(String, SettingOpts)> = current_settings.clone().into();

    Ok(list)
}
