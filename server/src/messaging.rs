use common::protocol::ServerMessage;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{broadcast, mpsc, RwLock};

/// Mappa user_id → canale diretto per messaggi privati.
pub type DirectChannels = Arc<RwLock<HashMap<i64, mpsc::Sender<ServerMessage>>>>;

// ── Broadcast ─────────────────────────────────────────────────────────────────

/// Invia un messaggio broadcast a tutti i client connessi.
pub fn send_broadcast(tx: &broadcast::Sender<ServerMessage>, content: String, from: String) {
    let msg = ServerMessage::BroadcastMessage { from, content };
    // Ignoriamo l'errore "nessun sottoscrittore"
    let _ = tx.send(msg);
}

// ── Direct message ────────────────────────────────────────────────────────────

/// Invia un messaggio diretto a un utente specifico.
/// Restituisce `true` se l'utente era connesso, `false` altrimenti.
pub async fn send_direct(
    channels: &DirectChannels,
    to_user_id: i64,
    content: String,
    from: String,
) -> bool {
    let ch = channels.read().await;
    if let Some(tx) = ch.get(&to_user_id) {
        tx.send(ServerMessage::DirectMessage { from, content })
            .await
            .is_ok()
    } else {
        false
    }
}

// ── Gestione canali ───────────────────────────────────────────────────────────

/// Registra il canale di un client appena autenticato.
pub async fn register_client(
    channels: &DirectChannels,
    user_id: i64,
    tx: mpsc::Sender<ServerMessage>,
) {
    channels.write().await.insert(user_id, tx);
}

/// Rimuove il canale di un client che si è disconnesso.
pub async fn unregister_client(channels: &DirectChannels, user_id: i64) {
    channels.write().await.remove(&user_id);
}
