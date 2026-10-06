/// Calcola la distanza in chilometri tra due coordinate GPS usando la formula di Haversine.
///
/// La formula di Haversine determina la distanza del cerchio massimo
/// tra due punti su una sfera (la Terra), date le loro coordinate
/// in gradi decimali (latitudine e longitudine).
///
/// # Parametri
/// - `lat1`, `lon1`: coordinate del primo punto (gradi decimali)
/// - `lat2`, `lon2`: coordinate del secondo punto (gradi decimali)
///
/// # Esempio
/// ```
/// use common::geo::haversine_km;
/// let d = haversine_km(45.0618513, 7.6606506, 44.9084148, 8.1778599);
/// assert!(d > 40.0 && d < 55.0); // Torino → Asti ≈ 47 km
/// ```
pub fn haversine_km(lat1: f64, lon1: f64, lat2: f64, lon2: f64) -> f64 {
    const EARTH_RADIUS_KM: f64 = 6371.0;

    let dlat = (lat2 - lat1).to_radians();
    let dlon = (lon2 - lon1).to_radians();
    let lat1_r = lat1.to_radians();
    let lat2_r = lat2.to_radians();

    let a = (dlat / 2.0).sin().powi(2) + lat1_r.cos() * lat2_r.cos() * (dlon / 2.0).sin().powi(2);

    // Clamp per evitare errori floating point (NaN da asin)
    let a = a.clamp(0.0, 1.0);
    let c = 2.0 * a.sqrt().asin();

    EARTH_RADIUS_KM * c
}

/// Calcola la velocità in km/h dati distanza (km) e tempo trascorso (secondi).
///
/// Restituisce 0.0 se il tempo è zero o negativo per evitare divisioni per zero.
pub fn speed_kmh(distance_km: f64, elapsed_secs: i64) -> f64 {
    if elapsed_secs <= 0 {
        return 0.0;
    }
    distance_km / (elapsed_secs as f64 / 3600.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haversine_torino_asti() {
        // Primo punto del dataset: Torino (45.0618513, 7.6606506)
        // Ultimo punto del dataset: Asti  (44.9084148, 8.1778599)
        // Distanza attesa: ~44-48 km
        let d = haversine_km(45.0618513, 7.6606506, 44.9084148, 8.1778599);
        assert!(d > 40.0 && d < 55.0, "Distanza inattesa: {:.2} km", d);
    }

    #[test]
    fn haversine_same_point() {
        // Due punti identici → distanza 0
        let d = haversine_km(45.0, 7.0, 45.0, 7.0);
        assert!(d.abs() < 0.001, "Stessa posizione dovrebbe dare 0 km");
    }

    #[test]
    fn speed_basic() {
        // 100 km in 3600 sec → 100 km/h
        let s = speed_kmh(100.0, 3600);
        assert!((s - 100.0).abs() < 0.01);
    }

    #[test]
    fn speed_zero_time() {
        assert_eq!(speed_kmh(10.0, 0), 0.0);
    }

    #[test]
    fn speed_negative_time() {
        assert_eq!(speed_kmh(10.0, -5), 0.0);
    }
}
