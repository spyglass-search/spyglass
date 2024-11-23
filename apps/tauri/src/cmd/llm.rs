use shared::llm::LlmSession;
use spyglass_rpc::RpcClient;
use tauri::Manager;

use crate::rpc;

#[tauri::command]
pub async fn ask_clippy(win: tauri::Window, session: LlmSession) -> Result<(), String> {
    tokio::spawn(async move {
        if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
            let rpc = rpc.lock().await;
            let _ = rpc.client.chat_completion(session).await;
        }
    });
    Ok(())
}
