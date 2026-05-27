use chrono::{Datelike, Local, TimeZone};
use common::geo;
use rusqlite::Connection;
use std::sync::{Arc, Mutex};

use crate::storage;

// ── Intervallo temporale ──────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TimeRange {
    Day,
    Week,
    Month,
}

impl TimeRange {
    /// Parsing da stringa (supporta italiano e inglese).
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "day" | "giorno" | "oggi"       => Some(TimeRange::Day),
            "week" | "settimana" | "w"      => Some(TimeRange::Week),
            "month" | "mese" | "m"          => Some(TimeRange::Month),
            _                               => None,
        }
    }

    /// Restituisce (start_unix, end_unix) dell'intervallo corrente.
    pub fn to_unix_range(&self) -> (i64, i64) {
        let now = Local::now();
        let end = now.timestamp();

        let start = match self {
            TimeRange::Day => {
                Local
                    .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
                    .unwrap()
                    .timestamp()
            }
            TimeRange::Week => {
                let days_back = now.weekday().num_days_from_monday() as i64;
                let monday = now.date_naive() - chrono::Duration::days(days_back);
                monday
                    .and_hms_opt(0, 0, 0)
                    .unwrap()
                    .and_local_timezone(Local)
                    .unwrap()
                    .timestamp()
            }
            TimeRange::Month => {
                Local
                    .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                    .unwrap()
                    .timestamp()
            }
        };
        (start, end)
    }

    pub fn label(&self) -> &'static str {
        match self {
            TimeRange::Day   => "giorno corrente",
            TimeRange::Week  => "settimana corrente",
            TimeRange::Month => "mese corrente",
        }
    }
}

// ── Query analytics ───────────────────────────────────────────────────────────

/// Restituisce il tragitto percorso come testo formattato.
pub fn get_route(db: &Arc<Mutex<Connection>>, user_id: i64, range: &TimeRange) -> String {
    let (start, end) = range.to_unix_range();
    let conn = db.lock().unwrap();

    match storage::get_positions_in_range(&conn, user_id, start, end) {
        Err(e) => format!("Errore DB: {e}"),
        Ok(positions) if positions.is_empty() => {
            format!("Nessuna posizione registrata nel {}.", range.label())
        }
        Ok(positions) => {
            let total_km: f64 = positions
                .windows(2)
                .map(|w| geo::haversine_km(w[0].lat, w[0].lon, w[1].lat, w[1].lon))
                .sum();

            let mut out = format!(
                "=== Tragitto ({}) — {} punti GPS, {:.2} km totali ===\n",
                range.label(),
                positions.len(),
                total_km
            );

            for (i, pos) in positions.iter().enumerate() {
                let t = chrono::DateTime::from_timestamp(pos.timestamp, 0)
                    .map(|dt| dt.with_timezone(&Local).format("%d/%m %H:%M:%S").to_string())
                    .unwrap_or_else(|| pos.timestamp.to_string());
                out += &format!("  [{:>3}] {} ({:.6}, {:.6})\n", i + 1, t, pos.lat, pos.lon);
            }
            out
        }
    }
}

/// Restituisce la velocità media come testo formattato.
pub fn get_avg_speed(db: &Arc<Mutex<Connection>>, user_id: i64, range: &TimeRange) -> String {
    let (start, end) = range.to_unix_range();
    let conn = db.lock().unwrap();

    match storage::get_positions_in_range(&conn, user_id, start, end) {
        Err(e) => format!("Errore DB: {e}"),
        Ok(positions) if positions.len() < 2 => {
            format!("Dati insufficienti per calcolare la velocità nel {}.", range.label())
        }
        Ok(positions) => {
            let total_km: f64 = positions
                .windows(2)
                .map(|w| geo::haversine_km(w[0].lat, w[0].lon, w[1].lat, w[1].lon))
                .sum();
            let total_secs =
                positions.last().unwrap().timestamp - positions.first().unwrap().timestamp;
            let speed = geo::speed_kmh(total_km, total_secs);

            format!(
                "=== Velocità media ({}) ===\n  Distanza: {:.2} km\n  Durata totale: {} min {} sec\n  Velocità media: {:.1} km/h\n",
                range.label(),
                total_km,
                total_secs / 60,
                total_secs % 60,
                speed
            )
        }
    }
}

/// Restituisce le pause (stazionamento) rilevate come testo formattato.
pub fn get_pauses(db: &Arc<Mutex<Connection>>, user_id: i64, range: &TimeRange) -> String {
    let (start, end) = range.to_unix_range();
    let conn = db.lock().unwrap();

    match storage::get_positions_in_range(&conn, user_id, start, end) {
        Err(e) => format!("Errore DB: {e}"),
        Ok(positions) if positions.len() < 2 => {
            format!("Dati insufficienti per rilevare pause nel {}.", range.label())
        }
        Ok(positions) => {
            let mut out = format!("=== Pause ({}) ===\n", range.label());
            let mut pause_count = 0u32;
            let mut total_pause_secs: i64 = 0;
            let mut i = 0usize;

            while i + 1 < positions.len() {
                let cur = &positions[i];
                let nxt = &positions[i + 1];
                let same = (cur.lat - nxt.lat).abs() < 1e-7
                    && (cur.lon - nxt.lon).abs() < 1e-7;

                if same {
                    // Estendi la pausa fino a che le coordinate cambiano
                    let pause_start = cur.timestamp;
                    let mut j = i + 1;
                    while j < positions.len() {
                        let prev = &positions[j - 1];
                        let p = &positions[j];
                        if (p.lat - prev.lat).abs() < 1e-7 && (p.lon - prev.lon).abs() < 1e-7 {
                            j += 1;
                        } else {
                            break;
                        }
                    }
                    let pause_end = positions[j - 1].timestamp;
                    let duration = pause_end - pause_start;

                    if duration >= 30 {
                        pause_count += 1;
                        total_pause_secs += duration;
                        let ts = |ts: i64| {
                            chrono::DateTime::from_timestamp(ts, 0)
                                .map(|dt| dt.with_timezone(&Local).format("%H:%M:%S").to_string())
                                .unwrap_or_else(|| ts.to_string())
                        };
                        out += &format!(
                            "  Pausa {:>2}: {} → {} ({} min {:02} sec) @ ({:.5},{:.5})\n",
                            pause_count,
                            ts(pause_start),
                            ts(pause_end),
                            duration / 60,
                            duration % 60,
                            cur.lat,
                            cur.lon
                        );
                    }
                    i = j;
                } else {
                    i += 1;
                }
            }

            if pause_count == 0 {
                out += "  Nessuna pausa rilevata.\n";
            } else {
                out += &format!(
                    "Totale: {} pause, {} min {} sec fermi\n",
                    pause_count,
                    total_pause_secs / 60,
                    total_pause_secs % 60
                );
            }
            out
        }
    }
}

// ── Parser comandi testuali ───────────────────────────────────────────────────

/// Interpreta un comando testuale inviato dal client (inizia con '/').
/// Restituisce la risposta come testo, o `None` se non riconosciuto.
pub fn handle_command(
    db: &Arc<Mutex<Connection>>,
    user_id: i64,
    command: &str,
) -> Option<String> {
    let parts: Vec<&str> = command.trim().splitn(3, ' ').collect();
    let range_str = parts.get(1).copied().unwrap_or("day");
    let range = TimeRange::from_str(range_str).unwrap_or(TimeRange::Day);

    match parts[0] {
        "/route" | "/tragitto"  => Some(get_route(db, user_id, &range)),
        "/speed" | "/velocita"  => Some(get_avg_speed(db, user_id, &range)),
        "/pauses" | "/pause"    => Some(get_pauses(db, user_id, &range)),
        "/help"                 => Some(help_text()),
        _                       => None,
    }
}

fn help_text() -> String {
    "=== Comandi disponibili ===\n\
     /route  [day|week|month]   — tragitto percorso\n\
     /speed  [day|week|month]   — velocità media\n\
     /pauses [day|week|month]   — durata delle pause\n\
     /help                      — questo messaggio\n"
        .to_string()
}
