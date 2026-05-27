mod auth;
mod emulator;
mod messaging;
mod network;

use emulator::{
    file::FileEmulator, manual::ManualEmulator, random::RandomEmulator, route::{Pause, RouteEmulator},
    Emulator,
};

use common::protocol::ClientMessage;
use std::{
    sync::{Arc, RwLock},
    time::Duration,
};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    sync::mpsc,
};

// ── Costanti ──────────────────────────────────────────────────────────────────

const SERVER_ADDR: &str = "127.0.0.1:7878";
const POSITION_INTERVAL_SECS: u64 = 30;

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    println!("╔════════════════════════════════════════╗");
    println!("║  Client Geolocalizzazione Flotta       ║");
    println!("╚════════════════════════════════════════╝");
    println!("Connessione a {} ...", SERVER_ADDR);

    let (conn, mut server_rx) = match network::connect(SERVER_ADDR).await {
        Ok(c) => {
            println!("✓ Connesso al server.");
            c
        }
        Err(e) => {
            eprintln!("✗ Impossibile connettersi: {e}");
            std::process::exit(1);
        }
    };

    // ── Autenticazione ────────────────────────────────────────────────────────
    let (user_id, username) = match auth::auth_menu(&conn.tx, &mut server_rx).await {
        Some(s) => s,
        None => {
            println!("Uscita.");
            return;
        }
    };

    // ── Scelta emulatore ──────────────────────────────────────────────────────
    let (emulator, manual_handle) = choose_emulator().await;

    // ── Task di ricezione messaggi ────────────────────────────────────────────
    tokio::spawn(messaging::start_message_printer(server_rx));

    // ── Task emulatore posizione ──────────────────────────────────────────────
    let tx_clone = conn.tx.clone();
    tokio::spawn(run_emulator(emulator, tx_clone, user_id));

    // ── Loop principale CLI ───────────────────────────────────────────────────
    main_loop(&conn.tx, &username, manual_handle).await;

    // ── Disconnessione ordinata ───────────────────────────────────────────────
    let _ = conn.tx.send(ClientMessage::Disconnect).await;
    println!("Disconnesso. Arrivederci!");
}

// ── Scelta della strategia di emulazione ─────────────────────────────────────

async fn choose_emulator() -> (Box<dyn Emulator>, Option<Arc<RwLock<Option<(f64, f64)>>>>) {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    println!("\n--- Strategia di emulazione del movimento ---");
    println!("  1. Lettura da file CSV");
    println!("  2. Rotta A→B interpolata con pause");
    println!("  3. Input manuale coordinate");
    println!("  4. Generatore pseudo-casuale");

    loop {
        let choice = prompt(&mut reader, "Scelta: ").await;
        match choice.as_str() {
            "1" => {
                let path = prompt(&mut reader, "Percorso file CSV: ").await;
                match FileEmulator::new(&path) {
                    Ok(e) => return (Box::new(e), None),
                    Err(e) => println!("Errore: {}. Riprova.", e),
                }
            }
            "2" => {
                println!("Inserisci le coordinate di partenza:");
                let lat_a = parse_f64(&prompt(&mut reader, "  Latitudine A: ").await);
                let lon_a = parse_f64(&prompt(&mut reader, "  Longitudine A: ").await);
                println!("Inserisci le coordinate di arrivo:");
                let lat_b = parse_f64(&prompt(&mut reader, "  Latitudine B: ").await);
                let lon_b = parse_f64(&prompt(&mut reader, "  Longitudine B: ").await);
                let speed = parse_f64_or(&prompt(&mut reader, "Velocità km/h (default 60): ").await, 60.0);

                let n_pauses: usize = parse_usize_or(&prompt(&mut reader, "Numero di fermate intermedie (0 = nessuna): ").await, 0);
                let mut pauses = Vec::new();
                for i in 0..n_pauses {
                    println!("  Fermata {}:", i + 1);
                    let frac = parse_f64_or(&prompt(&mut reader, "    Posizione nel percorso 0.0-1.0: ").await, 0.5).clamp(0.0, 1.0);
                    let dur = parse_u64_or(&prompt(&mut reader, "    Durata in secondi: ").await, 180);
                    pauses.push(Pause { at_fraction: frac, duration_secs: dur });
                }

                let e = RouteEmulator::new(lat_a, lon_a, lat_b, lon_b, speed, pauses);
                return (Box::new(e), None);
            }
            "3" => {
                println!("Inserisci la posizione iniziale:");
                let lat = parse_f64(&prompt(&mut reader, "  Latitudine: ").await);
                let lon = parse_f64(&prompt(&mut reader, "  Longitudine: ").await);
                let e = ManualEmulator::new(lat, lon);
                let handle = e.position_handle();
                println!(
                    "Modalità manuale: nel menu principale usa 'pos <lat> <lon>' per aggiornare la posizione."
                );
                return (Box::new(e), Some(handle));
            }
            "4" => {
                println!("Inserisci la posizione di partenza:");
                let lat = parse_f64(&prompt(&mut reader, "  Latitudine: ").await);
                let lon = parse_f64(&prompt(&mut reader, "  Longitudine: ").await);
                let step = parse_f64_or(&prompt(&mut reader, "Passo max in gradi (default 0.005 ≈ 500m): ").await, 0.005);
                let stop = parse_f64_or(&prompt(&mut reader, "Probabilità di fermarsi 0.0-1.0 (default 0.1): ").await, 0.1);
                let e = RandomEmulator::new(lat, lon, step, stop);
                return (Box::new(e), None);
            }
            _ => println!("Scelta non valida."),
        }
    }
}

// ── Task emulatore ────────────────────────────────────────────────────────────

async fn run_emulator(
    mut emulator: Box<dyn Emulator>,
    tx: mpsc::Sender<ClientMessage>,
    _user_id: i64,
) {
    println!(
        "\n[Emulatore avviato: {}]",
        emulator.description()
    );

    let mut interval = tokio::time::interval(Duration::from_secs(POSITION_INTERVAL_SECS));

    loop {
        interval.tick().await;

        match emulator.next_position() {
            Some((lat, lon)) => {
                let timestamp = chrono::Utc::now().timestamp();
                if tx
                    .send(ClientMessage::UpdatePosition { lat, lon, timestamp })
                    .await
                    .is_err()
                {
                    break;
                }
            }
            None => {
                println!("\n[Emulatore: percorso completato. Posizione invariata d'ora in poi.]");
                // Continua inviando l'ultima posizione (il server la tratterà come fermo)
                break;
            }
        }
    }
}

// ── Loop principale CLI ───────────────────────────────────────────────────────

async fn main_loop(
    tx: &mpsc::Sender<ClientMessage>,
    username: &str,
    manual_handle: Option<Arc<RwLock<Option<(f64, f64)>>>>,
) {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);

    println!("\n--- Menu principale (ciao, {}!) ---", username);
    println!("  msg <testo>              — invia messaggio al server");
    println!("  /route [day|week|month]  — tragitto percorso");
    println!("  /speed [day|week|month]  — velocità media");
    println!("  /pauses [day|week|month] — pause rilevate");
    println!("  /help                    — comandi analytics");
    if manual_handle.is_some() {
        println!("  pos <lat> <lon>          — aggiorna posizione (modalità manuale)");
    }
    println!("  quit                     — disconnetti\n");

    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            break;
        }
        let input = line.trim().to_string();
        if input.is_empty() {
            continue;
        }

        if input.eq_ignore_ascii_case("quit") || input.eq_ignore_ascii_case("exit") {
            break;
        }

        // Aggiornamento manuale posizione
        if let Some(ref handle) = manual_handle {
            if input.starts_with("pos ") {
                let parts: Vec<&str> = input.splitn(3, ' ').collect();
                if parts.len() == 3 {
                    let lat = parts[1].replace(',', ".").trim().parse::<f64>();
                    let lon = parts[2].replace(',', ".").trim().parse::<f64>();
                    match (lat, lon) {
                        (Ok(lat), Ok(lon)) => {
                            let mut pos = handle.write().unwrap();
                            *pos = Some((lat, lon));
                            println!("   Posizione aggiornata: ({:.6}, {:.6})", lat, lon);
                        }
                        _ => println!("Formato non valido. Usa: pos <lat> <lon>"),
                    }
                } else {
                    println!("Uso: pos <lat> <lon>");
                }
                continue;
            }
        }

        // Comandi analytics (iniziano con '/')
        if input.starts_with('/') {
            if tx
                .send(ClientMessage::SendMessage { content: input })
                .await
                .is_err()
            {
                eprintln!("Connessione persa.");
                break;
            }
            continue;
        }

        // Messaggio libero: rimuove prefisso "msg "
        let content = if input.starts_with("msg ") {
            input[4..].trim().to_string()
        } else {
            input.clone()
        };

        if content.is_empty() {
            println!("Messaggio vuoto.");
            continue;
        }

        if tx
            .send(ClientMessage::SendMessage { content })
            .await
            .is_err()
        {
            eprintln!("Connessione persa.");
            break;
        }
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

async fn prompt(reader: &mut BufReader<tokio::io::Stdin>, text: &str) -> String {
    use std::io::Write;
    print!("{}", text);
    std::io::stdout().flush().ok();
    let mut line = String::new();
    reader.read_line(&mut line).await.ok();
    line.trim().to_string()
}

fn parse_f64(s: &str) -> f64 {
    s.replace(',', ".").trim().parse().unwrap_or(0.0)
}

fn parse_f64_or(s: &str, default: f64) -> f64 {
    let v = s.replace(',', ".").trim().parse().unwrap_or(default);
    if v == 0.0 { default } else { v }
}

fn parse_usize_or(s: &str, default: usize) -> usize {
    s.trim().parse().unwrap_or(default)
}

fn parse_u64_or(s: &str, default: u64) -> u64 {
    s.trim().parse().unwrap_or(default)
}
