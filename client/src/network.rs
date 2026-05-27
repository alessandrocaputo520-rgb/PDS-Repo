use common::protocol::{recv_msg, send_msg, ClientMessage, ServerMessage};
use std::io;
use tokio::net::TcpStream;
use tokio::sync::mpsc;

// ── Connessione ───────────────────────────────────────────────────────────────

/// Handle per inviare messaggi al server.
pub struct Connection {
    pub tx: mpsc::Sender<ClientMessage>,
}

/// Connette al server e restituisce:
/// - `Connection` (per inviare messaggi)
/// - `mpsc::Receiver<ServerMessage>` (per ricevere messaggi)
pub async fn connect(addr: &str) -> io::Result<(Connection, mpsc::Receiver<ServerMessage>)> {
    let stream = TcpStream::connect(addr).await?;
    let (mut reader, mut writer) = stream.into_split();

    let (client_tx, mut client_rx) = mpsc::channel::<ClientMessage>(64);
    let (server_tx, server_rx) = mpsc::channel::<ServerMessage>(128);

    // Task di scrittura: prende messaggi dal canale e li invia sul socket
    tokio::spawn(async move {
        while let Some(msg) = client_rx.recv().await {
            let is_disconnect = matches!(msg, ClientMessage::Disconnect);
            if send_msg(&mut writer, &msg).await.is_err() {
                break;
            }
            if is_disconnect {
                break;
            }
        }
    });

    // Task di lettura: riceve dal socket e mette nel canale
    tokio::spawn(async move {
        loop {
            match recv_msg::<_, ServerMessage>(&mut reader).await {
                Ok(msg) => {
                    if server_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    Ok((Connection { tx: client_tx }, server_rx))
}
