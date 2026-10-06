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
        println!("Scegli emulatore: file examples/torino_asti.csv | manual 45.06 7.66 | random 45.06 7.66 | route 45.06 7.66 44.90 8.17 45 3");
        let Some(line) = input.next_line().await? else {
            return Ok(());
        };
        match emulator::parse_mode(&line) {
            Ok(e) => break e,
            Err(err) => eprintln!("{err}"),
        }
    };
    println!("GPS ogni 30s. Comandi: msg TESTO | report day|week|month | pos LAT LON (modo manuale) | quit");
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
                if let Some(value) = command.strip_prefix("pos ") {
                    let args: Vec<_> = value.split_whitespace().collect();
                    if args.len() == 2 {
                        if let (Ok(lat), Ok(lon)) = (args[0].parse::<f64>(), args[1].parse::<f64>()) {
                            if let Err(e) = emulator.set_manual(lat, lon) { eprintln!("{e}"); }
                            else { println!("Coordinate modificate (inviate al prossimo intervallo GPS)"); }
                            continue;
                        }
                    }
                    eprintln!("Uso: pos LAT LON");
                    continue;
                }
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
