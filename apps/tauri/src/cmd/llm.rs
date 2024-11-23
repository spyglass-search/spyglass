use shared::llm::LlmSession;
use spyglass_rpc::RpcClient;
use tauri::Manager;

use crate::rpc;

#[tauri::command]
pub async fn ask_clippy(win: tauri::Window, session: LlmSession) -> Result<(), String> {
    if let Some(rpc) = win.app_handle().try_state::<rpc::RpcMutex>() {
        let rpc = rpc.lock().await;
        if let Err(err) = rpc.client.chat_completion(session).await {
            return Err(err.to_string());
        }
    }

    Ok(())
}
