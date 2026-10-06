//! Interpolazione lineare A -> B con una pausa intermedia opzionale.
#[derive(Debug)]
pub struct Route {
    start: (f64, f64),
    end: (f64, f64),
    moving_steps: u64,
    pause_steps: u64,
    current: u64,
}
impl Route {
    pub fn new(start: (f64, f64), end: (f64, f64), minutes: u64, pause_minutes: u64) -> Self {
        Self {
            start,
            end,
            moving_steps: (minutes * 2).max(1),
            pause_steps: pause_minutes * 2,
            current: 0,
        }
    }
    pub fn next_position(&mut self) -> (f64, f64) {
        let midpoint = self.moving_steps / 2;
        let moving_index = if self.current <= midpoint {
            self.current
        } else if self.current <= midpoint + self.pause_steps {
            midpoint
        } else {
            self.current - self.pause_steps
        };
        let ratio = (moving_index as f64 / self.moving_steps as f64).min(1.0);
        self.current = self.current.saturating_add(1);
        (
            self.start.0 + (self.end.0 - self.start.0) * ratio,
            self.start.1 + (self.end.1 - self.start.1) * ratio,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stop_in_middle_and_arrive() {
        let mut r = Route::new((0., 0.), (10., 10.), 2, 1);
        let coords: Vec<_> = (0..8).map(|_| r.next_position()).collect();
        assert_eq!(coords[2], coords[3]);
        assert_eq!(coords[3], coords[4]);
        assert_eq!(coords[6], (10., 10.));
    }
}
