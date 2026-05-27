use super::Emulator;
use common::geo::haversine_km;

/// Emulatore che interpola linearmente da un punto A a un punto B.
///
/// Il percorso viene diviso in N step (ogni 30 secondi), con eventuali
/// pause (fermate) di durata configurabile.
pub struct RouteEmulator {
    /// Sequenza di posizioni pre-calcolate (lat, lon).
    waypoints: Vec<(f64, f64)>,
    index: usize,
}

/// Descrizione di una pausa lungo il percorso.
pub struct Pause {
    /// Progressione (0.0–1.0) nel percorso totale in cui si fa la pausa.
    pub at_fraction: f64,
    /// Durata della pausa in secondi.
    pub duration_secs: u64,
}

impl RouteEmulator {
    /// Crea l'emulatore interpolando da (lat_a, lon_a) a (lat_b, lon_b).
    ///
    /// - `speed_kmh`: velocità di crociera (km/h)
    /// - `pauses`: fermate intermedie (facoltative)
    pub fn new(
        lat_a: f64,
        lon_a: f64,
        lat_b: f64,
        lon_b: f64,
        speed_kmh: f64,
        pauses: Vec<Pause>,
    ) -> Self {
        const TICK_SECS: f64 = 30.0; // aggiornamento ogni 30 secondi

        let total_km = haversine_km(lat_a, lon_a, lat_b, lon_b);
        let travel_time_secs = (total_km / speed_kmh.max(1.0)) * 3600.0;
        let moving_ticks = (travel_time_secs / TICK_SECS).ceil() as usize;

        let mut waypoints: Vec<(f64, f64)> = Vec::new();

        let _current_tick = 0usize;
        let _total_ticks = moving_ticks
            + pauses
                .iter()
                .map(|p| (p.duration_secs as f64 / TICK_SECS).ceil() as usize)
                .sum::<usize>();

        // Raggruppa le pause per frazione di percorso
        let mut sorted_pauses = pauses;
        sorted_pauses.sort_by(|a, b| a.at_fraction.partial_cmp(&b.at_fraction).unwrap());

        let mut pause_iter = sorted_pauses.into_iter().peekable();

        for tick in 0..=moving_ticks {
            let t = tick as f64 / moving_ticks as f64; // 0.0 → 1.0
            let lat = lat_a + t * (lat_b - lat_a);
            let lon = lon_a + t * (lon_b - lon_a);
            waypoints.push((lat, lon));

            // Inserisci eventuali pause dopo questo waypoint
            while let Some(pause) = pause_iter.peek() {
                if pause.at_fraction <= t + (0.5 / moving_ticks as f64) {
                    let pause_ticks =
                        (pause.duration_secs as f64 / TICK_SECS).ceil() as usize;
                    for _ in 0..pause_ticks {
                        waypoints.push((lat, lon)); // posizione invariata = fermo
                    }
                    pause_iter.next();
                } else {
                    break;
                }
            }
        }

        let dist_approx = haversine_km(lat_a, lon_a, lat_b, lon_b);
        println!(
            "RouteEmulator: {:.2} km da ({:.4},{:.4}) a ({:.4},{:.4}) — {} tick @ {:.0} km/h",
            dist_approx,
            lat_a, lon_a,
            lat_b, lon_b,
            waypoints.len(),
            speed_kmh
        );

        Self { waypoints, index: 0 }
    }
}

impl Emulator for RouteEmulator {
    fn next_position(&mut self) -> Option<(f64, f64)> {
        if self.index < self.waypoints.len() {
            let pos = self.waypoints[self.index];
            self.index += 1;
            Some(pos)
        } else {
            None
        }
    }

    fn description(&self) -> &str {
        "Rotta A→B interpolata con pause"
    }
}
