//! Server TCP asincrono e interfaccia di controllo sulla console del server.
use crate::{
    analytics::{self, Period},
    auth,
    messaging::Hub,
    storage::Storage,
    tracker::Tracker,
};
use chrono::Utc;
use common::{
    models::Position,
    protocol::{recv_msg, send_msg, ClientMessage, ServerMessage},
};
use std::{
    io,
    sync::{Arc, Mutex},
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{mpsc, watch, Mutex as AsyncMutex},
};

pub struct AppState {
    pub db: Mutex<Storage>,
    pub tracker: Mutex<Tracker>,
    pub hub: AsyncMutex<Hub>,
}
impl AppState {
    pub fn new(db: Storage) -> Arc<Self> {
        Arc::new(Self {
            db: Mutex::new(db),
            tracker: Mutex::new(Tracker::default()),
            hub: AsyncMutex::new(Hub::default()),
        })
    }
}

fn io_error(s: impl ToString) -> io::Error {
    io::Error::other(s.to_string())
}

pub async fn serve(addr: &str, app: Arc<AppState>) -> io::Result<()> {
    let listener = TcpListener::bind(addr).await?;
    println!(
        "Server in ascolto su {}. Comandi: help",
        listener.local_addr()?
    );
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    tokio::spawn(admin_console(app.clone(), shutdown_tx));
    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => { println!("Arresto server."); break; }
            changed = shutdown_rx.changed() => {
                if changed.is_err() || *shutdown_rx.borrow() { break; }
            }
            res = listener.accept() => {
                let (stream, peer) = res?;
                let app = app.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(stream, app).await {
                        eprintln!("Connessione {peer}: {e}");
                    }
                });
            }
        }
    }
    Ok(())
}

async fn handle_connection(stream: TcpStream, app: Arc<AppState>) -> io::Result<()> {
    let (mut reader, mut writer) = stream.into_split();
    let (tx, mut rx) = mpsc::unbounded_channel::<ServerMessage>();
    let writer_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if send_msg(&mut writer, &msg).await.is_err() {
                break;
            }
        }
    });

    // Ogni connessione deve autenticarsi prima di aggiornare dati GPS.
    let identity = loop {
        let msg = match recv_msg::<_, ClientMessage>(&mut reader).await {
            Ok(m) => m,
            Err(e) => {
                drop(tx);
                writer_task.abort();
                return Err(e);
            }
        };
        let outcome = match msg {
            ClientMessage::Register { username, password } => {
                let db = app.db.lock().map_err(io_error)?;
                auth::register(&db, &username, &password, Utc::now().timestamp())
            }
            ClientMessage::Login { username, password } => {
                let db = app.db.lock().map_err(io_error)?;
                auth::login(&db, &username, &password)
            }
            ClientMessage::Disconnect => {
                drop(tx);
                writer_task.abort();
                return Ok(());
            }
            _ => Err("Devi prima registrarti o eseguire login".into()),
        };
        match outcome {
            Ok((id, name)) => {
                let inserted = { app.hub.lock().await.insert(id, name.clone(), tx.clone()) };
                if let Err(e) = inserted {
                    let _ = tx.send(ServerMessage::AuthFailure { reason: e });
                    continue;
                }
                // L'utente può avere dati storici dal precedente avvio.
                let previous = {
                    app.db
                        .lock()
                        .map_err(io_error)?
                        .latest_position(id)
                        .map_err(io_error)?
                        .map(|p| (p.lat, p.lon))
                };
                app.tracker
                    .lock()
                    .map_err(io_error)?
                    .connect(id, previous, Utc::now().timestamp());
                let _ = tx.send(ServerMessage::AuthSuccess {
                    user_id: id,
                    username: name.clone(),
                });
                // Consegna al login i messaggi diretti ricevuti mentre offline.
                let pending = app
                    .db
                    .lock()
                    .map_err(io_error)?
                    .undelivered(id)
                    .map_err(io_error)?;
                for msg in pending {
                    if tx
                        .send(ServerMessage::DirectMessage {
                            from: "SERVER (offline)".into(),
                            content: msg.content,
                        })
                        .is_ok()
                    {
                        app.db
                            .lock()
                            .map_err(io_error)?
                            .mark_delivered(msg.id, Utc::now().timestamp())
                            .map_err(io_error)?;
                    }
                }
                println!("[LOGIN] {name} (id={id})");
                break (id, name);
            }
            Err(reason) => {
                let _ = tx.send(ServerMessage::AuthFailure { reason });
            }
        }
    };

    let (id, name) = identity;
    loop {
        let incoming = recv_msg::<_, ClientMessage>(&mut reader).await;
        match incoming {
            Ok(ClientMessage::UpdatePosition {
                lat,
                lon,
                timestamp: _,
            }) => {
                // Timestamp lato server: il client non può falsificare la cronologia.
                if !lat.is_finite()
                    || !lon.is_finite()
                    || !(-90.0..=90.0).contains(&lat)
                    || !(-180.0..=180.0).contains(&lon)
                {
                    let _ = tx.send(ServerMessage::Error {
                        message: "Coordinate GPS non valide".into(),
                    });
                    continue;
                }
                let now = Utc::now().timestamp();
                let position = Position {
                    user_id: id,
                    lat,
                    lon,
                    timestamp: now,
                };
                let saved = { app.db.lock().map_err(io_error)?.add_position(&position) };
                if let Err(e) = saved {
                    let _ = tx.send(ServerMessage::Error {
                        message: format!("DB: {e}"),
                    });
                    continue;
                }
                let state = app
                    .tracker
                    .lock()
                    .map_err(io_error)?
                    .update(id, lat, lon, now);
                let _ = tx.send(ServerMessage::PositionAck { state });
            }
            Ok(ClientMessage::SendMessage { content }) => {
                let content = content.trim();
                if content.is_empty() || content.len() > 4096 {
                    let _ = tx.send(ServerMessage::Error {
                        message: "Messaggio vuoto o oltre 4096 byte".into(),
                    });
                    continue;
                }
                if let Some(period) = content
                    .strip_prefix("/report ")
                    .or_else(|| content.strip_prefix("report "))
                    .and_then(Period::parse)
                {
                    let report = user_report(&app, id, &name, period)?;
                    let _ = tx.send(ServerMessage::DirectMessage {
                        from: "SERVER".into(),
                        content: report,
                    });
                } else {
                    app.db
                        .lock()
                        .map_err(io_error)?
                        .add_message(Some(id), None, content, Utc::now().timestamp())
                        .map_err(io_error)?;
                    println!("[MSG] {name}: {content}");
                    let _ = tx.send(ServerMessage::DirectMessage {
                        from: "SERVER".into(),
                        content: "Messaggio ricevuto".into(),
                    });
                }
            }
            Ok(ClientMessage::Disconnect) | Err(_) => break,
            Ok(ClientMessage::Login { .. } | ClientMessage::Register { .. }) => {
                let _ = tx.send(ServerMessage::Error {
                    message: "Già autenticato".into(),
                });
            }
        }
    }
    app.hub.lock().await.remove(id);
    app.tracker.lock().map_err(io_error)?.disconnect(id);
    println!("[LOGOUT] {name}");
    drop(tx);
    writer_task.abort();
    Ok(())
}

fn user_report(app: &AppState, id: i64, name: &str, period: Period) -> io::Result<String> {
    let now = Utc::now();
    let positions = app
        .db
        .lock()
        .map_err(io_error)?
        .positions(id, period.since(now), now.timestamp())
        .map_err(io_error)?;
    Ok(analytics::format_report(
        name,
        period,
        &analytics::analyze(&positions),
    ))
}

async fn admin_console(app: Arc<AppState>, shutdown_tx: watch::Sender<bool>) {
    let mut lines = BufReader::new(tokio::io::stdin()).lines();
    println!("Comandi console: users | broadcast TESTO | to USER TESTO | report USER day|week|month | quit");
    while let Ok(Some(line)) = lines.next_line().await {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut args = line.split_whitespace();
        match args.next().unwrap_or("") {
            "help" => println!(
                "users | broadcast TESTO | to USER TESTO | report USER day|week|month | quit"
            ),
            "users" => {
                let users = app
                    .db
                    .lock()
                    .ok()
                    .and_then(|db| db.list_users().ok())
                    .unwrap_or_default();
                let online = app.hub.lock().await.online();
                for (id, name) in users {
                    let connected = online.iter().any(|(n, _)| *n == id);
                    let state = app
                        .tracker
                        .lock()
                        .ok()
                        .map(|t| t.state(id).to_string())
                        .unwrap_or_else(|| "errore".into());
                    println!(
                        "{name}: {} ({})",
                        state,
                        if connected { "online" } else { "offline" }
                    );
                }
            }
            "broadcast" => {
                let msg = line.strip_prefix("broadcast").unwrap_or("").trim();
                if msg.is_empty() {
                    println!("Uso: broadcast TESTO");
                    continue;
                }
                if let Ok(db) = app.db.lock() {
                    if let Err(e) = db.add_message(None, None, msg, Utc::now().timestamp()) {
                        eprintln!("DB: {e}");
                        continue;
                    }
                }
                let sent = app.hub.lock().await.broadcast("SERVER", msg);
                println!("Broadcast consegnato a {sent} connessioni");
            }
            "to" => {
                let Some(username) = args.next() else {
                    println!("Uso: to USER TESTO");
                    continue;
                };
                let content = line
                    .strip_prefix("to")
                    .unwrap_or("")
                    .trim()
                    .split_once(char::is_whitespace)
                    .map(|(_, content)| content.trim())
                    .unwrap_or("");
                if content.is_empty() {
                    println!("Uso: to USER TESTO");
                    continue;
                }
                let target = app
                    .db
                    .lock()
                    .ok()
                    .and_then(|db| db.find_user(username).ok().flatten());
                match target {
                    Some(user) => {
                        let stored = app.db.lock().ok().and_then(|db| {
                            db.add_message(None, Some(user.id), content, Utc::now().timestamp())
                                .ok()
                        });
                        let Some(message_id) = stored else {
                            eprintln!("Impossibile salvare il messaggio");
                            continue;
                        };
                        if app.hub.lock().await.direct(user.id, "SERVER", content) {
                            if let Ok(db) = app.db.lock() {
                                if let Err(e) =
                                    db.mark_delivered(message_id, Utc::now().timestamp())
                                {
                                    eprintln!("DB: {e}");
                                }
                            }
                            println!("Messaggio inviato a {}", user.username);
                        } else {
                            println!("Utente offline: messaggio in attesa di consegna al login");
                        }
                    }
                    None => println!("Utente inesistente"),
                }
            }
            "report" => {
                let (Some(name), Some(period)) = (args.next(), args.next().and_then(Period::parse))
                else {
                    println!("Uso: report USER day|week|month");
                    continue;
                };
                let target = app
                    .db
                    .lock()
                    .ok()
                    .and_then(|db| db.find_user(name).ok().flatten());
                match target {
                    Some(user) => match user_report(&app, user.id, &user.username, period) {
                        Ok(s) => println!("{s}"),
                        Err(e) => eprintln!("Report: {e}"),
                    },
                    None => println!("Utente inesistente"),
                }
            }
            "quit" | "exit" => {
                let _ = shutdown_tx.send(true);
                break;
            }
            _ => println!("Comando sconosciuto. Digita help"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn tcp_auth_gps_message_and_disconnect() {
        let app = AppState::new(Storage::open(":memory:").unwrap());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server_app = app.clone();
        let task = tokio::spawn(async move {
            let (conn, _) = listener.accept().await.unwrap();
            handle_connection(conn, server_app).await.unwrap();
        });
        let mut client = TcpStream::connect(addr).await.unwrap();
        send_msg(
            &mut client,
            &ClientMessage::Register {
                username: "alice".into(),
                password: "password-lunga".into(),
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            recv_msg::<_, ServerMessage>(&mut client).await.unwrap(),
            ServerMessage::AuthSuccess { .. }
        ));
        send_msg(
            &mut client,
            &ClientMessage::UpdatePosition {
                lat: 45.0,
                lon: 7.0,
                timestamp: 0,
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            recv_msg::<_, ServerMessage>(&mut client).await.unwrap(),
            ServerMessage::PositionAck {
                state: common::models::VehicleState::Stopped
            }
        ));
        send_msg(
            &mut client,
            &ClientMessage::UpdatePosition {
                lat: 45.1,
                lon: 7.0,
                timestamp: 0,
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            recv_msg::<_, ServerMessage>(&mut client).await.unwrap(),
            ServerMessage::PositionAck {
                state: common::models::VehicleState::Moving
            }
        ));
        send_msg(
            &mut client,
            &ClientMessage::SendMessage {
                content: "ciao".into(),
            },
        )
        .await
        .unwrap();
        assert!(matches!(
            recv_msg::<_, ServerMessage>(&mut client).await.unwrap(),
            ServerMessage::DirectMessage { .. }
        ));
        send_msg(&mut client, &ClientMessage::Disconnect)
            .await
            .unwrap();
        task.await.unwrap();
        assert_eq!(
            app.tracker.lock().unwrap().state(1),
            common::models::VehicleState::Disconnected
        );
        assert_eq!(
            app.db
                .lock()
                .unwrap()
                .positions(1, 0, Utc::now().timestamp())
                .unwrap()
                .len(),
            2
        );
    }
}
