mod analytics;
mod auth;
mod logger;
mod messaging;
mod network;
mod storage;
mod tracker;

use std::{env, error::Error};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::var("PDS_ADDR").unwrap_or_else(|_| "127.0.0.1:7878".into());
    let db_path = env::var("PDS_DB").unwrap_or_else(|_| "fleet.db".into());
    let log_path = env::var("PDS_CPU_LOG").unwrap_or_else(|_| "server_cpu.log".into());
    let db = storage::Storage::open(&db_path)?;
    println!("Database SQLite: {db_path}");
    tokio::spawn(logger::run(log_path));
    network::serve(&addr, network::AppState::new(db)).await?;
    Ok(())
}
