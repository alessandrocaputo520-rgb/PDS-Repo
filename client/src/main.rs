mod auth;
mod emulator;
mod messaging;
mod network;

use chrono::Utc;
use common::protocol::{recv_msg, ClientMessage, ServerMessage};
use std::{env, error::Error, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    time::MissedTickBehavior,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::var("PDS_ADDR").unwrap_or_else(|_| "127.0.0.1:7878".into());
    let (mut reader, writer) = network::connect(&addr).await?;
    println!("Connesso al server {addr}");
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    match auth::login(&mut input, &mut reader, &writer).await {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    let mut emulator = loop {
        println!("Inserisci il percorso del file CSV (es. file examples/torino_asti.csv):");
        let Some(line) = input.next_line().await? else {
            return Ok(());
        };
        match emulator::parse_mode(&line) {
            Ok(e) => break e,
            Err(err) => eprintln!("{err}"),
        }
    };
    println!("GPS ogni 30s. Comandi: msg TESTO | report day|week|month | quit");
    let mut ticks = tokio::time::interval(Duration::from_secs(30));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    loop {
        tokio::select! {
            _ = ticks.tick() => {
                let (lat, lon) = emulator.next_position();
                println!("[GPS] invio coordinate {lat:.6}, {lon:.6}");
                if let Err(e) = network::send(&writer, &ClientMessage::UpdatePosition { lat, lon, timestamp: Utc::now().timestamp() }).await {
                    eprintln!("Server disconnesso: {e}"); break;
                }
            }
            incoming = recv_msg::<_, ServerMessage>(&mut reader) => {
                match incoming {
                    Ok(msg) => messaging::display(msg),
                    Err(e) => { eprintln!("Server disconnesso: {e}"); break; }
                }
            }
            command = input.next_line() => {
                let Some(command) = command? else { break; };
                let command = command.trim();
                if command == "quit" || command == "exit" { break; }
                let content = command.strip_prefix("msg ").unwrap_or(command);
                if content.is_empty() { continue; }
                network::send(&writer, &ClientMessage::SendMessage { content: content.to_owned() }).await?;
            }
            _ = tokio::signal::ctrl_c() => { break; }
        }
    }
    let _ = network::send(&writer, &ClientMessage::Disconnect).await;
    println!("Client terminato");
    Ok(())
}
