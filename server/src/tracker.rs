//! Stato disconnesso/fermo/in movimento; la prima posizione è la baseline.
use common::models::VehicleState;
use std::collections::HashMap;

const STOP_AFTER_SECONDS: i64 = 180;
#[derive(Debug, Clone)]
struct Tracking {
    last_coord: Option<(f64, f64)>,
    last_change_at: i64,
    moving: bool,
}

#[derive(Default)]
pub struct Tracker {
    active: HashMap<i64, Tracking>,
}

impl Tracker {
    pub fn connect(&mut self, id: i64, previous: Option<(f64, f64)>, now: i64) {
        self.active.insert(
            id,
            Tracking {
                last_coord: previous,
                last_change_at: now,
                moving: false,
            },
        );
    }
    pub fn disconnect(&mut self, id: i64) {
        self.active.remove(&id);
    }
    pub fn update(&mut self, id: i64, lat: f64, lon: f64, now: i64) -> VehicleState {
        let Some(entry) = self.active.get_mut(&id) else {
            return VehicleState::Disconnected;
        };
        if let Some((old_lat, old_lon)) = entry.last_coord {
            if (old_lat, old_lon) != (lat, lon) {
                entry.moving = true;
                entry.last_change_at = now;
            } else if now.saturating_sub(entry.last_change_at) >= STOP_AFTER_SECONDS {
                entry.moving = false;
            }
        }
        entry.last_coord = Some((lat, lon));
        if entry.moving {
            VehicleState::Moving
        } else {
            VehicleState::Stopped
        }
    }
    pub fn state(&self, id: i64) -> VehicleState {
        match self.active.get(&id) {
            None => VehicleState::Disconnected,
            Some(t) if t.moving => VehicleState::Moving,
            Some(_) => VehicleState::Stopped,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn states_and_three_minute_timeout() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.state(1), VehicleState::Disconnected);
        tracker.connect(1, None, 0);
        assert_eq!(tracker.update(1, 45.0, 7.0, 0), VehicleState::Stopped);
        assert_eq!(tracker.update(1, 45.1, 7.0, 30), VehicleState::Moving);
        assert_eq!(tracker.update(1, 45.1, 7.0, 209), VehicleState::Moving);
        assert_eq!(tracker.update(1, 45.1, 7.0, 210), VehicleState::Stopped);
        assert_eq!(tracker.update(1, 45.2, 7.0, 240), VehicleState::Moving);
        tracker.disconnect(1);
        assert_eq!(tracker.state(1), VehicleState::Disconnected);
    }
}
