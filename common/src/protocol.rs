use crate::models::VehicleState;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ── Messaggi ──────────────────────────────────────────────────────────────────

/// Messaggi inviati dal client al server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMessage {
    /// Registrazione nuovo utente.
    Register { username: String, password: String },
    /// Login utente esistente.
    Login { username: String, password: String },
    /// Aggiornamento posizione GPS (inviato ogni 30 secondi).
    UpdatePosition { lat: f64, lon: f64, timestamp: i64 },
    /// Messaggio di testo libero (o comando che inizia con '/').
    SendMessage { content: String },
    /// Richiesta di disconnessione ordinata.
    Disconnect,
}

/// Messaggi inviati dal server al client.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ServerMessage {
    /// Autenticazione riuscita.
    AuthSuccess { user_id: i64, username: String },
    /// Autenticazione fallita.
    AuthFailure { reason: String },
    /// Conferma ricezione posizione con stato aggiornato.
    PositionAck { state: VehicleState },
    /// Messaggio broadcast (da server o a tutti gli utenti).
    BroadcastMessage { from: String, content: String },
    /// Messaggio diretto (risposta comandi analytics inclusa).
    DirectMessage { from: String, content: String },
    /// Errore generico.
    Error { message: String },
}

// ── Framing binario ───────────────────────────────────────────────────────────

/// Serializza e invia un messaggio sul writer con prefisso di lunghezza (4 byte BE).
pub async fn send_msg<W, T>(writer: &mut W, msg: &T) -> std::io::Result<()>
where
    W: AsyncWriteExt + Unpin,
    T: Serialize,
{
    let encoded = bincode::serialize(msg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    let len = encoded.len() as u32;
    writer.write_all(&len.to_be_bytes()).await?;
    writer.write_all(&encoded).await?;
    writer.flush().await?;
    Ok(())
}

/// Riceve e deserializza un messaggio dal reader (legge prefisso di lunghezza 4 byte BE).
pub async fn recv_msg<R, T>(reader: &mut R) -> std::io::Result<T>
where
    R: AsyncReadExt + Unpin,
    T: serde::de::DeserializeOwned,
{
    let mut len_buf = [0u8; 4];
    reader.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    // Limite di sicurezza: 10 MB per messaggio
    if len > 10 * 1024 * 1024 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Messaggio troppo grande: {} byte", len),
        ));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    bincode::deserialize(&buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
