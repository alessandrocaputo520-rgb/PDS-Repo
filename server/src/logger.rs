//! Campiona il tempo CPU del processo ogni 120 secondi (Linux, macOS).
use chrono::Utc;
use std::{fs::OpenOptions, io::Write, time::Duration};

#[cfg(unix)]
fn cpu_time_seconds() -> Option<f64> {
    // CLOCK_PROCESS_CPUTIME_ID: tempo CPU di tutti i thread del processo.
    unsafe {
        let mut ts: libc::timespec = std::mem::zeroed();
        if libc::clock_gettime(libc::CLOCK_PROCESS_CPUTIME_ID, &mut ts) != 0 {
            return None;
        }
        Some(ts.tv_sec as f64 + ts.tv_nsec as f64 / 1_000_000_000.0)
    }
}
#[cfg(not(unix))]
fn cpu_time_seconds() -> Option<f64> {
    None
}

pub async fn run(log_path: String) {
    let mut ticker = tokio::time::interval(Duration::from_secs(120));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    ticker.tick().await; // non scrivere subito, solo dopo 2 minuti
    let mut prev = cpu_time_seconds();
    loop {
        ticker.tick().await;
        let now = cpu_time_seconds();
        let delta = match (prev, now) {
            (Some(a), Some(b)) => format!("{:.3}", (b - a).max(0.0)),
            _ => "n/d".to_string(),
        };
        let total = now
            .map(|v| format!("{v:.3}"))
            .unwrap_or_else(|| "n/d".into());
        prev = now;
        let line = format!(
            "{} cpu_total_s={} cpu_last_120s_s={}\n",
            Utc::now().to_rfc3339(),
            total,
            delta
        );
        match OpenOptions::new().create(true).append(true).open(&log_path) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(line.as_bytes()) {
                    eprintln!("Log CPU: {e}");
                }
            }
            Err(e) => eprintln!("Impossibile aprire {log_path}: {e}"),
        }
    }
}
