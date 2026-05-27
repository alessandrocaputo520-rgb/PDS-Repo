use super::Emulator;
use std::sync::{Arc, RwLock};

/// Emulatore manuale: l'utente può aggiornare le coordinate in qualsiasi momento
/// dalla CLI. La posizione corrente viene inviata al prossimo tick (ogni 30 secondi).
pub struct ManualEmulator {
    /// Posizione condivisa con il thread principale della CLI.
    pub current: Arc<RwLock<Option<(f64, f64)>>>,
}

impl ManualEmulator {
    pub fn new(initial_lat: f64, initial_lon: f64) -> Self {
        Self {
            current: Arc::new(RwLock::new(Some((initial_lat, initial_lon)))),
        }
    }

    /// Restituisce un clone dell'Arc per permettere al thread CLI di aggiornare la posizione.
    pub fn position_handle(&self) -> Arc<RwLock<Option<(f64, f64)>>> {
        self.current.clone()
    }

    /// Aggiorna la posizione corrente (thread-safe).
    pub fn set_position(&self, lat: f64, lon: f64) {
        let mut pos = self.current.write().unwrap();
        *pos = Some((lat, lon));
    }
}

impl Emulator for ManualEmulator {
    /// Restituisce la posizione attualmente impostata dall'utente.
    /// Non restituisce mai `None` (l'emulazione manuale è infinita).
    fn next_position(&mut self) -> Option<(f64, f64)> {
        self.current.read().unwrap().clone()
    }

    fn description(&self) -> &str {
        "Input manuale coordinate"
    }
}
