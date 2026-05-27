use common::protocol::{ClientMessage, ServerMessage};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;

/// Menu interattivo di autenticazione.
/// Restituisce `(user_id, username)` in caso di successo, `None` se l'utente sceglie di uscire.
pub async fn auth_menu(
    tx: &mpsc::Sender<ClientMessage>,
    rx: &mut mpsc::Receiver<ServerMessage>,
) -> Option<(i64, String)> {
    let stdin = tokio::io::stdin();
    let mut reader = BufReader::new(stdin);
    let mut line = String::new();

    println!("\n╔════════════════════════════════╗");
    println!("║  Geolocalizzazione Flotta      ║");
    println!("╚════════════════════════════════╝");

    loop {
        println!("\n--- Autenticazione ---");
        println!("  1. Registrazione");
        println!("  2. Login");
        println!("  3. Esci");
        print!("Scelta: ");
        flush_stdout();

        line.clear();
        if reader.read_line(&mut line).await.unwrap_or(0) == 0 {
            return None;
        }

        match line.trim() {
            "1" => {
                let username = prompt_line(&mut reader, "Username: ").await;
                let password = prompt_line(&mut reader, "Password: ").await;

                if tx
                    .send(ClientMessage::Register { username, password })
                    .await
                    .is_err()
                {
                    eprintln!("Connessione persa.");
                    return None;
                }
            }
            "2" => {
                let username = prompt_line(&mut reader, "Username: ").await;
                let password = prompt_line(&mut reader, "Password: ").await;

                if tx
                    .send(ClientMessage::Login { username, password })
                    .await
                    .is_err()
                {
                    eprintln!("Connessione persa.");
                    return None;
                }
            }
            "3" => return None,
            _ => {
                println!("Scelta non valida.");
                continue;
            }
        }

        // Attendi risposta del server (timeout 15 secondi)
        match tokio::time::timeout(std::time::Duration::from_secs(15), rx.recv()).await {
            Ok(Some(ServerMessage::AuthSuccess { user_id, username })) => {
                println!("\n✓ Accesso effettuato come '{}'!", username);
                return Some((user_id, username));
            }
            Ok(Some(ServerMessage::AuthFailure { reason })) => {
                println!("✗ Errore: {}", reason);
            }
            Ok(Some(ServerMessage::Error { message })) => {
                println!("✗ Server error: {}", message);
            }
            Ok(_) => println!("Risposta inattesa dal server."),
            Err(_) => println!("Timeout: nessuna risposta dal server."),
        }
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

async fn prompt_line(
    reader: &mut BufReader<tokio::io::Stdin>,
    prompt: &str,
) -> String {
    print!("{}", prompt);
    flush_stdout();
    let mut line = String::new();
    reader.read_line(&mut line).await.ok();
    line.trim().to_string()
}

fn flush_stdout() {
    use std::io::Write;
    std::io::stdout().flush().ok();
}
