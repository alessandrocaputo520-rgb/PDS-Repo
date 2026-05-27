use common::protocol::ServerMessage;
use tokio::sync::mpsc;

/// Stampa un messaggio server in modo formattato.
pub fn print_server_message(msg: &ServerMessage) {
    match msg {
        ServerMessage::BroadcastMessage { from, content } => {
            println!("\n📢 [BROADCAST da {}]: {}", from, content);
        }
        ServerMessage::DirectMessage { from, content } => {
            println!("\n✉  [DA {}]:\n{}", from, content);
        }
        ServerMessage::PositionAck { state } => {
            println!("   📍 Posizione ricevuta — stato: {}", state);
        }
        ServerMessage::Error { message } => {
            eprintln!("\n⚠  Errore server: {}", message);
        }
        _ => {}
    }
}

/// Task in background: legge i messaggi in arrivo dal server e li stampa.
pub async fn start_message_printer(mut server_rx: mpsc::Receiver<ServerMessage>) {
    while let Some(msg) = server_rx.recv().await {
        print_server_message(&msg);
    }
    println!("\n[Connessione al server chiusa]");
}
