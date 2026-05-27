use std::{fs::OpenOptions, io::Write, time::Duration};
use sysinfo::{Pid, System};

/// Task in background: ogni 2 minuti registra il consumo CPU del processo server
/// nel file `server_cpu.log`.
pub async fn start_cpu_logger() {
    let mut interval = tokio::time::interval(Duration::from_secs(120));
    let mut sys = System::new();
    let pid = Pid::from_u32(std::process::id());

    log::info!("CPU logger avviato (intervallo 2 minuti) → server_cpu.log");

    loop {
        interval.tick().await;

        // Aggiorna le informazioni del processo corrente
        sys.refresh_process(pid);

        let cpu = sys
            .process(pid)
            .map(|p| p.cpu_usage())
            .unwrap_or(0.0);

        let ts = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let line = format!("[{}] PID={} CPU={:.2}%\n", ts, std::process::id(), cpu);

        match OpenOptions::new()
            .create(true)
            .append(true)
            .open("server_cpu.log")
        {
            Ok(mut f) => {
                if let Err(e) = f.write_all(line.as_bytes()) {
                    log::error!("Errore scrittura server_cpu.log: {e}");
                }
            }
            Err(e) => log::error!("Impossibile aprire server_cpu.log: {e}"),
        }

        log::info!("CPU usage: {:.2}%", cpu);
    }
}
