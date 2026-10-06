//! L'utente usa 'pos lat lon' per cambiare coordinate, inviate ogni 30 s.
#[derive(Debug)]
pub struct Manual {
    pub position: (f64, f64),
}
impl Manual {
    pub fn new(lat: f64, lon: f64) -> Self {
        Self {
            position: (lat, lon),
        }
    }
    pub fn next_position(&self) -> (f64, f64) {
        self.position
    }
}
