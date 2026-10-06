//! Statistiche per giorno, settimana ISO (lunedì), mese correnti, fuso UTC.
use chrono::{DateTime, Datelike, TimeZone, Utc};
use common::{
    geo::{haversine_km, speed_kmh},
    models::Position,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Day,
    Week,
    Month,
}
impl Period {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "day" | "oggi" | "giorno" => Some(Self::Day),
            "week" | "settimana" => Some(Self::Week),
            "month" | "mese" => Some(Self::Month),
            _ => None,
        }
    }
    pub fn since(self, now: DateTime<Utc>) -> i64 {
        let d = now.date_naive();
        let first = match self {
            Period::Day => d,
            Period::Week => d - chrono::Duration::days(d.weekday().num_days_from_monday() as i64),
            Period::Month => d.with_day(1).expect("giorno 1 valido"),
        };
        Utc.from_utc_datetime(&first.and_hms_opt(0, 0, 0).expect("mezzanotte"))
            .timestamp()
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Day => "oggi",
            Self::Week => "settimana",
            Self::Month => "mese",
        }
    }
}

#[derive(Debug, Default)]
pub struct Report {
    pub samples: usize,
    pub total_km: f64,
    pub moving_seconds: i64,
    pub paused_seconds: i64,
    pub average_moving_kmh: f64,
    pub route: Vec<(f64, f64)>,
}

pub fn analyze(samples: &[Position]) -> Report {
    let mut r = Report {
        samples: samples.len(),
        ..Report::default()
    };
    if let Some(first) = samples.first() {
        r.route.push((first.lat, first.lon));
    }
    for pair in samples.windows(2) {
        let (a, b) = (&pair[0], &pair[1]);
        let elapsed = (b.timestamp - a.timestamp).max(0);
        let distance = haversine_km(a.lat, a.lon, b.lat, b.lon);
        if distance > 0.000001 {
            r.total_km += distance;
            r.moving_seconds += elapsed;
            r.route.push((b.lat, b.lon));
        } else {
            r.paused_seconds += elapsed;
        }
    }
    r.average_moving_kmh = speed_kmh(r.total_km, r.moving_seconds);
    r
}

pub fn format_report(username: &str, period: Period, r: &Report) -> String {
    let path = if r.route.is_empty() {
        "Nessuna posizione".to_owned()
    } else if r.route.len() <= 60 {
        r.route
            .iter()
            .map(|(lat, lon)| format!("{lat:.5},{lon:.5}"))
            .collect::<Vec<_>>()
            .join(" -> ")
    } else {
        let start: Vec<_> = r.route.iter().take(30).collect();
        let end: Vec<_> = r.route.iter().skip(r.route.len() - 30).collect();
        start
            .into_iter()
            .map(|(lat, lon)| format!("{lat:.5},{lon:.5}"))
            .chain(std::iter::once(format!(
                "... {} punti intermedi ...",
                r.route.len() - 60
            )))
            .chain(
                end.into_iter()
                    .map(|(lat, lon)| format!("{lat:.5},{lon:.5}")),
            )
            .collect::<Vec<_>>()
            .join(" -> ")
    };
    format!("Utente: {username} | Periodo: {} (UTC)\nCampioni: {}\nDistanza: {:.3} km\nVelocità media durante il movimento: {:.2} km/h\nTempo movimento: {} s\nTempo pause: {} s\nTragitto: {}",
       period.label(), r.samples, r.total_km, r.average_moving_kmh,
       r.moving_seconds, r.paused_seconds, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn p(lat: f64, lon: f64, t: i64) -> Position {
        Position {
            user_id: 1,
            lat,
            lon,
            timestamp: t,
        }
    }
    #[test]
    fn movement_and_pause() {
        let r = analyze(&[p(45.0, 7.0, 0), p(45.0, 7.0, 30), p(45.01, 7.0, 60)]);
        assert_eq!(r.paused_seconds, 30);
        assert_eq!(r.moving_seconds, 30);
        assert!(r.total_km > 1.0);
        assert_eq!(r.route.len(), 2);
    }
    #[test]
    fn period_boundaries() {
        let t = Utc.with_ymd_and_hms(2026, 10, 6, 15, 0, 0).unwrap();
        assert_eq!(
            Period::Day.since(t),
            Utc.with_ymd_and_hms(2026, 10, 6, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
        assert_eq!(
            Period::Week.since(t),
            Utc.with_ymd_and_hms(2026, 10, 5, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
        assert_eq!(
            Period::Month.since(t),
            Utc.with_ymd_and_hms(2026, 10, 1, 0, 0, 0)
                .unwrap()
                .timestamp()
        );
    }
}
