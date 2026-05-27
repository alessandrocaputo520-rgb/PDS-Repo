mod analytics;
mod auth;
mod logger;
mod messaging;
mod network;
mod storage;
mod tracker;

use common::models::VehicleState;
use common::protocol::ServerMessage;
use messaging::DirectChannels;
use rusqlite::Connection;
use std::{collections::HashMap, sync::{Arc, Mutex}};
use tokio::sync::{broadcast, RwLock};
use tracker::TrackerState;

// ── Stato condiviso dell'applicazione ────────────────────────────────────────

pub struct AppState {
    pub db: Arc<Mutex<Connection>>,
    pub tracker: Arc<TrackerState>,
    pub broadcast_tx: broadcast::Sender<ServerMessage>,
    pub direct_channels: DirectChannels,
    /// Mappa user_id → username per gli utenti attualmente connessi.
    pub connected_users: Arc<RwLock<HashMap<i64, String>>>,
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    // Inizializza logger (variabile d'ambiente RUST_LOG, default "info")
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    println!("╔══════════════════════════════════════════╗");
    println!("║  Server Geolocalizzazione Flotta Veicoli ║");
    println!("╚══════════════════════════════════════════╝");

    // Database SQLite
    let db = storage::init_db("fleet.db").expect("Impossibile inizializzare il database SQLite");
    let db = Arc::new(Mutex::new(db));
    log::info!("Database SQLite aperto: fleet.db");

    // Canale broadcast (capacità 256 messaggi)
    let (broadcast_tx, _) = broadcast::channel::<ServerMessage>(256);

    let state = Arc::new(AppState {
        db,
        tracker: Arc::new(TrackerState::new()),
        broadcast_tx,
        direct_channels: Arc::new(RwLock::new(HashMap::new())),
        connected_users: Arc::new(RwLock::new(HashMap::new())),
    });

    // Task in background: monitor stato veicoli
    tokio::spawn(tracker::start_state_monitor(state.tracker.clone()));

    // Task in background: log CPU ogni 2 minuti
    tokio::spawn(logger::start_cpu_logger());

    // CLI admin in background
    let state_cli = state.clone();
    tokio::spawn(async move {
        server_cli(state_cli).await;
    });

    // TCP listener (blocca fino alla fine)
    network::start_listener(state).await;
}

// ── CLI amministratore ────────────────────────────────────────────────────────

async fn server_cli(state: Arc<AppState>) {
    use tokio::io::{AsyncBufReadExt, BufReader};

    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    println!("\nComandi server disponibili:");
    println!("  list                              — utenti connessi");
    println!("  broadcast <messaggio>             — messaggio a tutti");
    println!("  dm <username> <messaggio>         — messaggio diretto");
    println!("  analytics <username> <comando>    — analytics (route/speed/pauses [day/week/month])");
    println!("  quit                              — chiudi il server\n");

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            break;
        }
        let trimmed = line.trim().to_string();
        if trimmed.is_empty() {
            continue;
        }

        let (cmd, rest) = trimmed
            .split_once(' ')
            .map(|(c, r)| (c, r.trim()))
            .unwrap_or((&trimmed, ""));

        match cmd {
            "list" => {
                let users = state.connected_users.read().await;
                if users.is_empty() {
                    println!("Nessun utente connesso.");
                } else {
                    println!("=== Utenti connessi ({}) ===", users.len());
                    let states = tracker::get_all_states(&state.tracker).await;
                    let state_map: HashMap<i64, VehicleState> = states.into_iter().collect();
                    for (id, name) in users.iter() {
                        let st = state_map
                            .get(id)
                            .cloned()
                            .unwrap_or(VehicleState::Disconnected);
                        println!("  [{:>4}] {:20} — {}", id, name, st);
                    }
                }
            }

            "broadcast" => {
                if rest.is_empty() {
                    println!("Uso: broadcast <messaggio>");
                } else {
                    messaging::send_broadcast(
                        &state.broadcast_tx,
                        rest.to_string(),
                        "Server".to_string(),
                    );
                    // Salva nel DB
                    let conn = state.db.lock().unwrap();
                    let _ = storage::insert_message(&conn, None, None, rest);
                    println!("Broadcast inviato.");
                }
            }

            "dm" => {
                let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                if parts.len() < 2 {
                    println!("Uso: dm <username> <messaggio>");
                } else {
                    let username = parts[0];
                    let content = parts[1].trim();
                    let user_id = {
                        let conn = state.db.lock().unwrap();
                        storage::find_user_id_by_username(&conn, username)
                            .ok()
                            .flatten()
                    };
                    match user_id {
                        Some(uid) => {
                            let sent = messaging::send_direct(
                                &state.direct_channels,
                                uid,
                                content.to_string(),
                                "Server".to_string(),
                            )
                            .await;
                            if sent {
                                // Salva nel DB
                                let conn = state.db.lock().unwrap();
                                let _ = storage::insert_message(&conn, None, Some(uid), content);
                                println!("Messaggio inviato a '{}'.", username);
                            } else {
                                println!("'{}' non è attualmente connesso.", username);
                            }
                        }
                        None => println!("Utente '{}' non trovato.", username),
                    }
                }
            }

            "analytics" => {
                let parts: Vec<&str> = rest.splitn(2, ' ').collect();
                if parts.is_empty() {
                    println!("Uso: analytics <username> <route|speed|pauses> [day|week|month]");
                } else {
                    let username = parts[0];
                    let sub_cmd = parts.get(1).map(|s| format!("/{s}")).unwrap_or_else(|| "/help".to_string());
                    let user_id = {
                        let conn = state.db.lock().unwrap();
                        storage::find_user_id_by_username(&conn, username)
                            .ok()
                            .flatten()
                    };
                    match user_id {
                        Some(uid) => {
                            match analytics::handle_command(&state.db, uid, &sub_cmd) {
                                Some(result) => println!("{}", result),
                                None => println!("Comando non riconosciuto: {}", sub_cmd),
                            }
                        }
                        None => println!("Utente '{}' non trovato.", username),
                    }
                }
            }

            "quit" | "exit" => {
                println!("Server in chiusura...");
                std::process::exit(0);
            }

            _ => println!("Comando non riconosciuto. Usa: list, broadcast, dm, analytics, quit"),
        }
    }
}
