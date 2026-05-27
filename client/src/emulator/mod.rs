pub mod file;
pub mod manual;
pub mod random;
pub mod route;

// ── Trait Emulatore ───────────────────────────────────────────────────────────

/// Interfaccia comune per tutte le strategie di emulazione del movimento.
pub trait Emulator: Send {
    /// Restituisce la prossima posizione (lat, lon), o `None` se la simulazione è terminata.
    fn next_position(&mut self) -> Option<(f64, f64)>;

    /// Descrizione testuale della strategia.
    fn description(&self) -> &str;
}
