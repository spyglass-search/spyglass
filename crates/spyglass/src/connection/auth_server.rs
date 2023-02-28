use std::net::SocketAddr;
use std::{collections::HashMap, convert::Infallible};
use tokio::sync::{broadcast, mpsc};
use warp::Filter;

type AuthChannel = mpsc::Sender<AuthCode>;
type ShutdownChannel = broadcast::Sender<()>;

#[derive(Clone, Debug, Default)]
pub struct AuthCode {
    pub scopes: Vec<String>,
    pub code: String,
    pub state: String,
}

pub struct AuthListener {
    addr: SocketAddr,
    code_rx: mpsc::Receiver<AuthCode>,
    shutdown_tx: ShutdownChannel,
}

impl AuthListener {
    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub async fn listen(&mut self, timeout_s: u64) -> Option<AuthCode> {
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(timeout_s)) => {
                let _ = self.shutdown_tx.send(());
                None
            },
            code = self.code_rx.recv() => {
                let _ = self.shutdown_tx.send(());
                code
            }
        }
    }
}

/// Handle redirect request for OAuth flow
pub async fn handle_request(
    code_tx: AuthChannel,
    p: HashMap<String, String>,
) -> Result<(), String> {
    log::debug!("received: {:?}", p);

    let mut auth = AuthCode::default();
    if let Some(scope) = p.get("scope") {
        auth.scopes = scope.split(' ').map(|s| s.to_string()).collect();
    }

    if let Some(state) = p.get("state") {
        auth.state = state.clone();
    }

    if let Some(code) = p.get("code") {
        auth.code = code.clone();
    }

    if let Err(err) = code_tx.send(auth).await {
        Err(err.to_string())
    } else {
        Ok(())
    }
}

pub fn with_channel(
    chan: AuthChannel,
) -> impl Filter<Extract = (AuthChannel,), Error = Infallible> + Clone {
    warp::any().map(move || chan.clone())
}

/// Starts a basic HTTP server and listens for authentication requests
pub async fn create_auth_listener(port: Option<u16>) -> AuthListener {
    let (tx, _) = broadcast::channel::<()>(1);

    let (code_tx, code_rx) = mpsc::channel::<AuthCode>(1);

    let capture = warp::get()
        .and(with_channel(code_tx))
        .and(warp::query::<HashMap<String, String>>())
        .and_then(|chan: AuthChannel, p: HashMap<String, String>| async move {
            if let Err(err) = handle_request(chan.clone(), p.clone()).await {
                log::error!("Unable to send auth code: {}", err);
                Err(warp::reject::reject())
            } else {
                Ok("Authenticated! You can close this window/tab now. 🙂".to_string())
            }
        });

    let tx_clone = tx.clone();
    let (addr, server) = warp::serve(capture).bind_with_graceful_shutdown(
        ([127, 0, 0, 1], port.unwrap_or_default()),
        async move {
            let mut rx = tx_clone.subscribe();
            let _ = rx.recv().await;
        },
    );

    tokio::task::spawn(server);

    AuthListener {
        addr,
        code_rx,
        shutdown_tx: tx,
    }
}
