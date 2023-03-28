use crate::window::notify;
use crate::{rpc, AppEvent};
use anyhow::anyhow;
use jsonrpsee::core::client::Subscription;
use spyglass_rpc::{ModelDownloadStatusPayload, RpcClient, RpcEvent, RpcEventType};
use tauri::{
    async_runtime::JoinHandle,
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
use tokio::sync::broadcast;

pub struct NotificationHandler(JoinHandle<()>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("tauri-plugin-notify")
        .on_event(|app_handle, event| match event {
            RunEvent::Ready => {
                log::info!("starting notify plugin");
                let handle =
                    tauri::async_runtime::spawn(setup_notification_handler(app_handle.clone()));
                app_handle.manage(NotificationHandler(handle));
            }
            RunEvent::Exit => {
                let app_handle = app_handle.clone();
                if let Some(handle) = app_handle.try_state::<NotificationHandler>() {
                    handle.0.abort();
                }
            }
            _ => {}
        })
        .build()
}

async fn _subscribe(app: &AppHandle) -> anyhow::Result<Subscription<RpcEvent>> {
    let rpc = match app.try_state::<rpc::RpcMutex>() {
        Some(rpc) => rpc,
        None => return Err(anyhow!("Server not available")),
    };

    let rpc = rpc.lock().await;
    let sub = rpc
        .client
        .subscribe_events(vec![
            RpcEventType::ConnectionSyncFinished,
            RpcEventType::LensInstalled,
            RpcEventType::LensUninstalled,
            RpcEventType::ModelDownloadStatus,
        ])
        .await?;

    Ok(sub)
}

async fn setup_notification_handler(app: AppHandle) {
    let app_events = app.state::<broadcast::Sender<AppEvent>>();
    let mut channel = app_events.subscribe();

    // wait for RPC server connection
    log::info!("waiting for backend...");
    match channel.recv().await {
        Ok(AppEvent::BackendConnected) => {}
        _ => return,
    }

    let mut sub = match _subscribe(&app).await {
        Ok(sub) => sub,
        Err(err) => {
            log::warn!("Unable to subscribe to backend events: {err}");
            return;
        }
    };

    log::info!("subscribed to events from server!");
    loop {
        tokio::select! {
            event = channel.recv() => {
                if let Ok(AppEvent::Shutdown) = event {
                    log::info!("🛑 Shutting down notify plugin");
                    return;
                }
            },
            event = sub.next() => {
                match event {
                    Some(Ok(event)) =>  {
                        log::debug!("received event: {:?}", event);
                        let notif: Option<(String, String)> = match &event.event_type {
                            RpcEventType::ConnectionSyncFinished => Some(("Sync Completed".into(), event.payload)),
                            RpcEventType::LensInstalled => Some(("Lens Installed".into(), event.payload)),
                            RpcEventType::LensUninstalled => Some(("Lens Removed".into(), event.payload)),
                            RpcEventType::ModelDownloadStatus => {
                                if let Ok(status) = serde_json::de::from_str::<ModelDownloadStatusPayload>(&event.payload) {
                                    match status {
                                        ModelDownloadStatusPayload::Finished { model_name } => {
                                            let window = crate::window::update_progress_window(&app, &model_name, 100);
                                            let _ = window.close();

                                            Some((
                                                "Model Installed".into(),
                                                format!("Finished downloading {}", model_name)
                                            ))
                                        },
                                        ModelDownloadStatusPayload::Error { model_name, msg } => {
                                            Some((
                                                "Model Download Failed".into(),
                                                format!("Unable to download {} model: {}", model_name, msg)
                                            ))
                                        },
                                        ModelDownloadStatusPayload::InProgress { model_name, percent } => {
                                            log::info!("downloading: {} - {}", model_name, percent);
                                            crate::window::update_progress_window(&app, &model_name, percent);
                                            None
                                        }
                                    }
                                } else {
                                    None
                                }
                            }
                        };

                        if let Some((title, blurb)) = notif {
                            let _ = notify(&app, &title, &blurb);
                        }
                    },
                    Some(Err(err)) => log::warn!("error listening to event: {:?}", err),
                    // channel dropped, attempt to reconnect
                    None => {
                        sub = match _subscribe(&app).await {
                            Ok(sub) => sub,
                            Err(err) => {
                                log::warn!("Unable to subscribe to backend events: {err}");
                                return;
                            }
                        };
                    }
                }
            }
        }
    }
}
