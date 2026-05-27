use common::models::VehicleState;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

/// Soglia di inattività per passare a "Fermo" (3 minuti).
pub const STOP_THRESHOLD: Duration = Duration::from_secs(180);

// ── Stato condiviso del tracker ───────────────────────────────────────────────

pub struct TrackerState {
    /// Stato corrente per ogni utente.
    pub vehicle_states: Arc<RwLock<HashMap<i64, VehicleState>>>,
    /// Ultima posizione nota per ogni utente (lat, lon).
    pub last_positions: Arc<RwLock<HashMap<i64, (f64, f64)>>>,
    /// Istante dell'ultimo cambio di coordinate.
    pub last_change: Arc<RwLock<HashMap<i64, Instant>>>,
}

impl TrackerState {
    pub fn new() -> Self {
        Self {
            vehicle_states: Arc::new(RwLock::new(HashMap::new())),
            last_positions: Arc::new(RwLock::new(HashMap::new())),
            last_change: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for TrackerState {
    fn default() -> Self {
        Self::new()
    }
}

// ── API pubblica ──────────────────────────────────────────────────────────────

/// Chiamata al login: l'utente viene messo in stato Moving.
pub async fn set_connected(tracker: &TrackerState, user_id: i64) {
    let mut states = tracker.vehicle_states.write().await;
    states.insert(user_id, VehicleState::Moving);
    let mut times = tracker.last_change.write().await;
    times.entry(user_id).or_insert_with(Instant::now);
}

/// Chiamata alla disconnessione TCP.
pub async fn set_disconnected(tracker: &TrackerState, user_id: i64) {
    let mut states = tracker.vehicle_states.write().await;
    states.insert(user_id, VehicleState::Disconnected);
    let mut positions = tracker.last_positions.write().await;
    positions.remove(&user_id);
}

/// Aggiorna la posizione e applica la macchina a stati:
/// - coordinate cambiate → Moving, reset timer
/// - coordinate invariate → controlla timeout 3 min → eventuale Stopped
/// Restituisce lo stato risultante.
pub async fn update_position(
    tracker: &TrackerState,
    user_id: i64,
    lat: f64,
    lon: f64,
) -> VehicleState {
    let mut positions = tracker.last_positions.write().await;
    let mut states = tracker.vehicle_states.write().await;
    let mut times = tracker.last_change.write().await;

    let moved = match positions.get(&user_id) {
        Some(&(old_lat, old_lon)) => {
            (lat - old_lat).abs() > 1e-7 || (lon - old_lon).abs() > 1e-7
        }
        // Prima posizione ricevuta → consideriamo come "mossa"
        None => true,
    };

    positions.insert(user_id, (lat, lon));

    if moved {
        states.insert(user_id, VehicleState::Moving);
        times.insert(user_id, Instant::now());
    } else {
        let current = states.entry(user_id).or_insert(VehicleState::Moving);
        if *current == VehicleState::Moving {
            let elapsed = times
                .get(&user_id)
                .map(|t| t.elapsed())
                .unwrap_or(Duration::ZERO);
            if elapsed >= STOP_THRESHOLD {
                *current = VehicleState::Stopped;
                log::info!(
                    "Utente {} → Fermo (inattivo da {:.0}s)",
                    user_id,
                    elapsed.as_secs_f64()
                );
            }
        }
    }

    states
        .get(&user_id)
        .cloned()
        .unwrap_or(VehicleState::Moving)
}

/// Restituisce lo stato corrente di un utente.
pub async fn get_state(tracker: &TrackerState, user_id: i64) -> VehicleState {
    let states = tracker.vehicle_states.read().await;
    states
        .get(&user_id)
        .cloned()
        .unwrap_or(VehicleState::Disconnected)
}

/// Restituisce la lista di tutti gli stati attivi.
pub async fn get_all_states(tracker: &TrackerState) -> Vec<(i64, VehicleState)> {
    let states = tracker.vehicle_states.read().await;
    states.iter().map(|(&id, s)| (id, s.clone())).collect()
}

// ── Task di monitoraggio in background ───────────────────────────────────────

/// Controlla ogni 10 secondi se un utente Moving è fermo da più di 3 minuti.
pub async fn start_state_monitor(tracker: Arc<TrackerState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    log::info!("Monitor stato veicoli avviato (intervallo 10s)");

    loop {
        interval.tick().await;

        let times = tracker.last_change.read().await;
        let mut to_stop: Vec<i64> = Vec::new();

        for (&user_id, &instant) in times.iter() {
            if instant.elapsed() >= STOP_THRESHOLD {
                let states = tracker.vehicle_states.read().await;
                if states.get(&user_id) == Some(&VehicleState::Moving) {
                    to_stop.push(user_id);
                }
            }
        }
        drop(times);

        if !to_stop.is_empty() {
            let mut states = tracker.vehicle_states.write().await;
            for uid in to_stop {
                log::info!("Monitor: utente {} → Fermo", uid);
                states.insert(uid, VehicleState::Stopped);
            }
        }
    }
}
