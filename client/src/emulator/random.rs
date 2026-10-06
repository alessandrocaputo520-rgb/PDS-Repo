//! Random walk deterministico con xorshift; include soste di almeno 3 minuti.
#[derive(Debug)]
pub struct Random {
    coords: (f64, f64),
    state: u64,
    ticks: u64,
}
impl Random {
    pub fn new(lat: f64, lon: f64) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;
        Self {
            coords: (lat, lon),
            state: seed.max(1),
            ticks: 0,
        }
    }
    fn sample(&mut self) -> f64 {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as f64 / u64::MAX as f64) * 2.0 - 1.0
    }
    pub fn next_position(&mut self) -> (f64, f64) {
        // 10 step in moto, 8 step di pausa, periodicamente.
        if self.ticks % 18 < 10 && self.ticks != 0 {
            let dlat = self.sample() * 0.0005;
            let dlon = self.sample() * 0.0005;
            self.coords.0 = (self.coords.0 + dlat).clamp(-90.0, 90.0);
            self.coords.1 = (self.coords.1 + dlon).clamp(-180.0, 180.0);
        }
        self.ticks += 1;
        self.coords
    }
}
