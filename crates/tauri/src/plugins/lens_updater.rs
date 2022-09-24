use tauri::{
    async_runtime::JoinHandle,
    plugin::{Builder, TauriPlugin},
    AppHandle, Manager, RunEvent, Wry,
};
use tokio::signal;
use tokio::time::{self, Duration};

pub struct LensWatcherHandle(JoinHandle<()>);

pub fn init() -> TauriPlugin<Wry> {
    Builder::new("tauri-plugin-lens-updater")
        .on_event(|app_handle, event| match event {
            RunEvent::Ready => {
                let app_handle = app_handle.clone();
                let app_clone = app_handle.clone();
                let handle = tauri::async_runtime::spawn(async move {
                    let mut interval = time::interval(Duration::from_secs(10));
                    let app_handle = app_handle.clone();
                    loop {
                        tokio::select! {
                            _ = signal::ctrl_c() => break,
                            _ = interval.tick() => check_for_lens_updates(&app_handle).await,
                        }
                    }
                });

                app_clone.manage(LensWatcherHandle(handle));
            }
            RunEvent::Exit => {
                let app_handle = app_handle.clone();
                if let Some(handle) = app_handle.try_state::<LensWatcherHandle>() {
                    handle.0.abort();
                }
            }
            _ => {}
        })
        .build()
}

async fn check_for_lens_updates(_app_handle: &AppHandle) {
    todo!();
}
