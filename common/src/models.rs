use serde::{Deserialize, Serialize};

// ── User ─────────────────────────────────────────────────────────────────────

/// Un utente registrato nel sistema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    pub password_hash: String,
    pub created_at: i64, // Unix timestamp (secondi)
}

// ── Position ──────────────────────────────────────────────────────────────────

/// Una misurazione GPS di un utente.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Position {
    pub user_id: i64,
    pub lat: f64,
    pub lon: f64,
    pub timestamp: i64, // Unix timestamp (secondi)
}

// ── VehicleState ──────────────────────────────────────────────────────────────

/// Stato della macchina a stati del veicolo.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VehicleState {
    /// TCP disconnesso.
    Disconnected,
    /// Coordinate cambiate almeno una volta dall'ultimo fermo.
    Moving,
    /// Coordinate invariate per più di 3 minuti.
    Stopped,
}

impl std::fmt::Display for VehicleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VehicleState::Disconnected => write!(f, "Sconnesso"),
            VehicleState::Moving      => write!(f, "In movimento"),
            VehicleState::Stopped     => write!(f, "Fermo"),
        }
    }
}

// ── Message ───────────────────────────────────────────────────────────────────

/// Messaggio di testo scambiato nel sistema.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: i64,
    /// `None` = mittente è il server.
    pub from_user_id: Option<i64>,
    /// `None` = broadcast.
    pub to_user_id: Option<i64>,
    pub content: String,
    pub timestamp: i64,
}
