mod auth;
mod emulator;
mod messaging;
mod network;
mod ui;

use chrono::Utc;
use common::protocol::{recv_msg, ClientMessage, ServerMessage};
use crossterm::{
    event::{self, EventStream, KeyCode, KeyEventKind},
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
    ExecutableCommand,
};
use futures::StreamExt;
use ratatui::prelude::{CrosstermBackend, Terminal};
use std::{env, error::Error, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    time::MissedTickBehavior,
};
use ui::AppState;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let addr = env::var("PDS_ADDR").unwrap_or_else(|_| "127.0.0.1:7878".into());
    let (mut reader, writer) = network::connect(&addr).await?;
    println!("Connesso al server {addr}");
    
    let mut input = BufReader::new(tokio::io::stdin()).lines();
    
    // Auth Testuale
    match auth::login(&mut input, &mut reader, &writer).await {
        Ok(_) => (),
        Err(e) if e.kind() == std::io::ErrorKind::Interrupted => return Ok(()),
        Err(e) => return Err(e.into()),
    }
    
    // Il server invia un AuthSuccess che possiamo ricevere per ottenere lo username (per semplicità in UI)
    // Ma auth::login legge già il messaggio di AuthSuccess o Failure internamente.
    // Usiamo uno username generico o "Utente" per ora, o potremmo estrarlo.
    let username = "Utente".to_string(); // In futuro auth::login potrebbe restituirlo

    // CSV Testuale
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
    
    let csv_path = "CSV Locale".to_string(); // Si potrebbe estrarre il path esatto se emulator lo esponesse

    // ---- INIZIO TUI ----
    enable_raw_mode()?;
    std::io::stdout().execute(EnterAlternateScreen)?;
    let mut terminal = Terminal::new(CrosstermBackend::new(std::io::stdout()))?;

    let mut app = AppState::new(username, addr, csv_path);
    app.push_log("Avvio sistema GPS...".into());

    let mut ticks = tokio::time::interval(Duration::from_secs(1));
    ticks.set_missed_tick_behavior(MissedTickBehavior::Skip);
    
    let mut crossterm_events = EventStream::new();
    
    let mut gps_timer = 30; // invia ogni 30s
    let (start_lat, start_lon) = emulator.next_position(); // first position
    app.lat = start_lat;
    app.lon = start_lon;

    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        tokio::select! {
            _ = ticks.tick() => {
                if gps_timer == 0 {
                    let (lat, lon) = emulator.next_position();
                    app.lat = lat;
                    app.lon = lon;
                    app.push_log(format!("[GPS] Inviate coordinate {:.6}, {:.6}", lat, lon));
                    
                    if let Err(_) = network::send(&writer, &ClientMessage::UpdatePosition { lat, lon, timestamp: Utc::now().timestamp() }).await {
                        app.push_log("[ERRORE] Server disconnesso!".into());
                        break;
                    }
                    gps_timer = 30;
                } else {
                    gps_timer -= 1;
                }
                app.next_gps_in_secs = gps_timer;
            }
            
            incoming = recv_msg::<_, ServerMessage>(&mut reader) => {
                match incoming {
                    Ok(msg) => app.handle_server_message(msg),
                    Err(_) => {
                        app.push_log("[ERRORE] Connessione al server persa!".into());
                        break;
                    }
                }
            }
            
            ev = crossterm_events.next() => {
                match ev {
                    Some(Ok(event::Event::Key(key))) => {
                        if key.kind == KeyEventKind::Press {
                            match key.code {
                                KeyCode::Esc => break,
                                KeyCode::Char('c') if key.modifiers.contains(event::KeyModifiers::CONTROL) => break,
                                KeyCode::Char(c) => app.input.push(c),
                                KeyCode::Backspace => { app.input.pop(); }
                                KeyCode::Enter => {
                                    if !app.input.is_empty() {
                                        let cmd = app.input.clone();
                                        app.input.clear();
                                        let _ = network::send(&writer, &ClientMessage::SendMessage { content: cmd }).await;
                                    }
                                }
                                KeyCode::F(1) => {
                                    let _ = network::send(&writer, &ClientMessage::SendMessage { content: "report day".into() }).await;
                                }
                                KeyCode::F(2) => {
                                    let _ = network::send(&writer, &ClientMessage::SendMessage { content: "report week".into() }).await;
                                }
                                KeyCode::Up => {
                                    if app.log_scroll + 1 < app.logs.len() {
                                        app.log_scroll += 1;
                                    }
                                }
                                KeyCode::Down => {
                                    if app.log_scroll > 0 {
                                        app.log_scroll -= 1;
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // ---- FINE TUI ----
    disable_raw_mode()?;
    std::io::stdout().execute(LeaveAlternateScreen)?;
    
    let _ = network::send(&writer, &ClientMessage::Disconnect).await;
    println!("Client terminato");
    Ok(())
}
