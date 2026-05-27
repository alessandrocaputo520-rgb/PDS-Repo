use super::Emulator;
use rand::{rngs::StdRng, Rng, SeedableRng};

/// Emulatore pseudo-casuale: genera un "random walk" partendo da una posizione iniziale.
///
/// Ogni tick sposta le coordinate di una piccola quantità casuale (max ±`step_deg` gradi),
/// simulando un veicolo che si muove in modo non deterministico.
pub struct RandomEmulator {
    lat: f64,
    lon: f64,
    step_deg: f64,
    /// Probabilità (0.0–1.0) che il veicolo rimanga fermo al prossimo tick.
    stop_prob: f64,
    rng: StdRng,
}

impl RandomEmulator {
    /// - `lat`, `lon`: posizione iniziale
    /// - `step_deg`: massimo spostamento in gradi per tick (default suggerito: 0.005 ≈ 500 m)
    /// - `stop_prob`: probabilità di rimanere fermo (0.0 = sempre in moto, 0.3 = fermo 30% dei tick)
    pub fn new(lat: f64, lon: f64, step_deg: f64, stop_prob: f64) -> Self {
        Self {
            lat,
            lon,
            step_deg: step_deg.max(0.0001),
            stop_prob: stop_prob.clamp(0.0, 1.0),
            rng: StdRng::from_entropy(),
        }
    }
}

impl Emulator for RandomEmulator {
    fn next_position(&mut self) -> Option<(f64, f64)> {
        // Con probabilità stop_prob il veicolo rimane fermo
        if self.rng.gen::<f64>() < self.stop_prob {
            return Some((self.lat, self.lon));
        }

        // Spostamento casuale in tutte le direzioni
        let dlat = self.rng.gen_range(-self.step_deg..=self.step_deg);
        let dlon = self.rng.gen_range(-self.step_deg..=self.step_deg);

        self.lat = (self.lat + dlat).clamp(-90.0, 90.0);
        self.lon = (self.lon + dlon).clamp(-180.0, 180.0);

        Some((self.lat, self.lon))
    }

    fn description(&self) -> &str {
        "Generatore pseudo-casuale di coordinate"
    }
}
