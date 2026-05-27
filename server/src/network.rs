use std::sync::Arc;

use common::protocol::{recv_msg, send_msg, ClientMessage, ServerMessage};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{broadcast, mpsc};

use crate::{analytics, auth, messaging, storage, tracker, AppState};

// ── Entry point ───────────────────────────────────────────────────────────────

/// Avvia il listener TCP sulla porta 7878 e gestisce le connessioni in arrivo.
pub async fn start_listener(state: Arc<AppState>) {
    let listener = TcpListener::bind("0.0.0.0:7878")
        .await
        .expect("Impossibile avviare il listener TCP sulla porta 7878");

    println!("🚀 Server in ascolto su 0.0.0.0:7878");
    log::info!("TCP listener avviato su 0.0.0.0:7878");

    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                log::info!("Nuova connessione da {}", addr);
                let state = state.clone();
                tokio::spawn(handle_client(socket, addr.to_string(), state));
            }
            Err(e) => log::error!("Errore accept: {e}"),
        }
    }
}

// ── Gestione client ───────────────────────────────────────────────────────────

async fn handle_client(socket: TcpStream, addr: String, state: Arc<AppState>) {
    let (mut reader, mut writer) = socket.into_split();

    // ── Fase di autenticazione ────────────────────────────────────────────────
    let session = match auth_phase(&mut reader, &mut writer, &state).await {
        Ok(s) => s,
        Err(e) => {
            log::warn!("[{}] Auth fallita: {}", addr, e);
            return;
        }
    };
    let (user_id, username) = session;
    log::info!("[{}] Utente '{}' (id={}) autenticato", addr, username, user_id);

    // ── Registrazione canali ──────────────────────────────────────────────────
    let (direct_tx, mut direct_rx) = mpsc::channel::<ServerMessage>(64);
    messaging::register_client(&state.direct_channels, user_id, direct_tx.clone()).await;
    tracker::set_connected(&state.tracker, user_id).await;
    state
        .connected_users
        .write()
        .await
        .insert(user_id, username.clone());

    // ── Task di scrittura ─────────────────────────────────────────────────────
    // Unisce messaggi diretti + broadcast e li scrive sul socket.
    let mut broadcast_rx = state.broadcast_tx.subscribe();
    let write_task = tokio::spawn(async move {
        loop {
            tokio::select! {
                Some(msg) = direct_rx.recv() => {
                    if send_msg(&mut writer, &msg).await.is_err() { break; }
                }
                result = broadcast_rx.recv() => {
                    match result {
                        Ok(msg) => {
                            if send_msg(&mut writer, &msg).await.is_err() { break; }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            log::warn!("Client '{}' ha perso {} messaggi broadcast", user_id, n);
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                else => break,
            }
        }
    });

    // ── Loop di lettura ───────────────────────────────────────────────────────
    loop {
        match recv_msg::<_, ClientMessage>(&mut reader).await {
            Ok(msg) => {
                let cont = handle_message(msg, user_id, &username, &state, &direct_tx).await;
                if !cont {
                    break;
                }
            }
            Err(e) => {
                if e.kind() != std::io::ErrorKind::UnexpectedEof {
                    log::warn!("[{}] Errore lettura: {}", addr, e);
                }
                break;
            }
        }
    }

    // ── Cleanup ───────────────────────────────────────────────────────────────
    write_task.abort();
    messaging::unregister_client(&state.direct_channels, user_id).await;
    tracker::set_disconnected(&state.tracker, user_id).await;
    state.connected_users.write().await.remove(&user_id);
    log::info!("[{}] Utente '{}' disconnesso", addr, username);
}

// ── Fase di autenticazione ────────────────────────────────────────────────────

async fn auth_phase(
    reader: &mut tokio::net::tcp::OwnedReadHalf,
    writer: &mut tokio::net::tcp::OwnedWriteHalf,
    state: &Arc<AppState>,
) -> Result<(i64, String), String> {
    for attempt in 1..=3 {
        let msg = recv_msg::<_, ClientMessage>(reader)
            .await
            .map_err(|e| e.to_string())?;

        match msg {
            ClientMessage::Register { username, password } => {
                match auth::register(&state.db, &username, &password) {
                    Ok(user_id) => {
                        send_msg(
                            writer,
                            &ServerMessage::AuthSuccess {
                                user_id,
                                username: username.clone(),
                            },
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        log::info!("Nuovo utente registrato: '{}' (id={})", username, user_id);
                        return Ok((user_id, username));
                    }
                    Err(reason) => {
                        let _ = send_msg(writer, &ServerMessage::AuthFailure { reason }).await;
                    }
                }
            }
            ClientMessage::Login { username, password } => {
                match auth::login(&state.db, &username, &password) {
                    Ok(user) => {
                        send_msg(
                            writer,
                            &ServerMessage::AuthSuccess {
                                user_id: user.id,
                                username: user.username.clone(),
                            },
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                        return Ok((user.id, user.username));
                    }
                    Err(reason) => {
                        let _ = send_msg(writer, &ServerMessage::AuthFailure { reason }).await;
                    }
                }
            }
            _ => {
                let _ = send_msg(
                    writer,
                    &ServerMessage::Error {
                        message: "Autenticarsi prima (Register o Login)".to_string(),
                    },
                )
                .await;
            }
        }

        if attempt == 3 {
            return Err("Troppi tentativi di autenticazione falliti".to_string());
        }
    }
    unreachable!()
}

// ── Gestione messaggi client ──────────────────────────────────────────────────

/// Restituisce `false` se il client ha richiesto la disconnessione.
async fn handle_message(
    msg: ClientMessage,
    user_id: i64,
    username: &str,
    state: &Arc<AppState>,
    direct_tx: &mpsc::Sender<ServerMessage>,
) -> bool {
    match msg {
        ClientMessage::UpdatePosition { lat, lon, timestamp } => {
            // Salva nel DB (dentro un blocco per rilasciare il lock prima dell'await)
            {
                let conn = state.db.lock().unwrap();
                if let Err(e) = storage::insert_position(&conn, user_id, lat, lon, timestamp) {
                    log::error!("Errore salvataggio posizione utente {}: {}", user_id, e);
                }
            }
            // Aggiorna la macchina a stati
            let new_state = tracker::update_position(&state.tracker, user_id, lat, lon).await;
            log::debug!(
                "Posizione utente '{}': ({:.6}, {:.6}) → {}",
                username,
                lat,
                lon,
                new_state
            );
            // Conferma al client
            let _ = direct_tx
                .send(ServerMessage::PositionAck { state: new_state })
                .await;
        }

        ClientMessage::SendMessage { content } => {
            if content.starts_with('/') {
                // Comando analytics
                match analytics::handle_command(&state.db, user_id, &content) {
                    Some(response) => {
                        let _ = direct_tx
                            .send(ServerMessage::DirectMessage {
                                from: "Server".to_string(),
                                content: response,
                            })
                            .await;
                    }
                    None => {
                        let _ = direct_tx
                            .send(ServerMessage::DirectMessage {
                                from: "Server".to_string(),
                                content: format!(
                                    "Comando '{}' non riconosciuto. Usa /help.",
                                    content.split_whitespace().next().unwrap_or(&content)
                                ),
                            })
                            .await;
                    }
                }
            } else {
                // Messaggio testuale libero → salva e stampa a server
                {
                    let conn = state.db.lock().unwrap();
                    let _ = storage::insert_message(&conn, Some(user_id), None, &content);
                }
                log::info!("[MSG] {}: {}", username, content);
                println!("[MSG] {}: {}", username, content);
            }
        }

        ClientMessage::Disconnect => {
            return false;
        }

        _ => {
            // Register/Login ignorati dopo l'auth
        }
    }
    true
}
