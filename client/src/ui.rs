use common::models::VehicleState;
use common::protocol::ServerMessage;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style, Modifier},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Gauge, List, ListItem},
    Frame,
};
use std::collections::VecDeque;
use chrono::{DateTime, Local, TimeZone};

pub struct AppState {
    pub username: String,
    pub server_addr: String,
    pub csv_path: String,

    // GPS State
    pub lat: f64,
    pub lon: f64,
    pub vehicle_state: VehicleState,
    pub next_gps_in_secs: u64,

    // Logs
    pub logs: VecDeque<(DateTime<Local>, String)>,
    
    // Input
    pub input: String,
    
    // Log scrolling
    pub log_scroll: usize,
}

impl AppState {
    pub fn new(username: String, server_addr: String, csv_path: String) -> Self {
        Self {
            username,
            server_addr,
            csv_path,
            lat: 0.0,
            lon: 0.0,
            vehicle_state: VehicleState::Disconnected,
            next_gps_in_secs: 30,
            logs: VecDeque::new(),
            input: String::new(),
            log_scroll: 0,
        }
    }

    pub fn push_log(&mut self, msg: String) {
        self.logs.push_back((Local::now(), msg));
        if self.logs.len() > 100 {
            self.logs.pop_front();
        }
        // Se l'utente non sta scorrendo attivamente in su, teniamo lo scroll a 0 per mostrare i più recenti
        if self.log_scroll == 0 {
            // scroll follows bottom
        }
    }

    pub fn handle_server_message(&mut self, msg: ServerMessage) {
        match msg {
            ServerMessage::PositionAck { state } => {
                self.vehicle_state = state.clone();
                self.push_log(format!("[GPS] stato: {}", state));
            }
            ServerMessage::BroadcastMessage { from, content } => {
                self.push_log(format!("[BROADCAST] {}: {}", from, content));
            }
            ServerMessage::DirectMessage { from, content } => {
                self.push_log(format!("[MESSAGGIO] {}: {}", from, content));
            }
            ServerMessage::AuthFailure { reason } => {
                self.push_log(format!("Autenticazione: {}", reason));
            }
            ServerMessage::AuthSuccess { username, .. } => {
                self.push_log(format!("Connesso come {}", username));
            }
            ServerMessage::Error { message } => {
                self.push_log(format!("[ERRORE] {}", message));
            }
        }
    }
}

pub fn draw(f: &mut Frame, app: &AppState) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),      // Header
            Constraint::Length(6),      // Dashboard (GPS + Stato)
            Constraint::Min(5),         // Logs
            Constraint::Length(3),      // Input
        ])
        .split(f.area());

    // 1. Header
    let header_text = format!(" 🚗 Fleet GPS Client — {} | Server: {} | CSV: {} ", app.username, app.server_addr, app.csv_path);
    let header = Paragraph::new(header_text)
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(header, chunks[0]);

    // 2. Dashboard
    let dash_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(chunks[1]);

    // Stato Veicolo & Countdown
    let state_color = match app.vehicle_state {
        VehicleState::Moving => Color::Green,
        VehicleState::Stopped => Color::Yellow,
        VehicleState::Disconnected => Color::Red,
    };
    
    let state_text = vec![
        Line::from(vec![
            Span::raw("Stato: "),
            Span::styled(format!("{}", app.vehicle_state), Style::default().fg(state_color).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!("Prossimo invio in: {}s", app.next_gps_in_secs)),
    ];
    let state_par = Paragraph::new(state_text)
        .block(Block::default().title(" Stato Veicolo ").borders(Borders::ALL));
    f.render_widget(state_par, dash_chunks[0]);
    
    let progress = (30u64.saturating_sub(app.next_gps_in_secs)) as f64 / 30.0;
    let gauge = Gauge::default()
        .block(Block::default().borders(Borders::NONE))
        .gauge_style(Style::default().fg(Color::Blue))
        .ratio(progress.clamp(0.0, 1.0));
    
    // Posizioniamo la gauge dentro il blocco stato, magari usando un layout interno
    let inner_state_area = dash_chunks[0].inner(ratatui::layout::Margin { vertical: 1, horizontal: 1 });
    let gauge_area = Layout::default().direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner_state_area)[1];
    f.render_widget(gauge, gauge_area);

    // Coordinate GPS
    let gps_text = vec![
        Line::from(format!("Lat: {:.6}", app.lat)),
        Line::from(format!("Lon: {:.6}", app.lon)),
    ];
    let gps_par = Paragraph::new(gps_text)
        .block(Block::default().title(" Coordinate Attuali ").borders(Borders::ALL));
    f.render_widget(gps_par, dash_chunks[1]);

    // 3. Logs
    let log_messages: Vec<ListItem> = app.logs.iter().rev()
        .map(|(ts, msg)| {
            let time_str = ts.format("%H:%M:%S").to_string();
            let content = Line::from(vec![
                Span::styled(format!("[{}] ", time_str), Style::default().fg(Color::DarkGray)),
                Span::raw(msg),
            ]);
            ListItem::new(content)
        })
        .collect();

    let logs_list = List::new(log_messages)
        .block(Block::default().title(" Log Eventi (scorrere con ↑/↓) ").borders(Borders::ALL));
    
    // Calcoliamo lo scroll
    let mut list_state = ratatui::widgets::ListState::default();
    if !app.logs.is_empty() {
        list_state.select(Some(app.log_scroll));
    }
    
    f.render_stateful_widget(logs_list, chunks[2], &mut list_state);

    // 4. Input Command
    let input_text = format!("msg {}_", app.input);
    let input_par = Paragraph::new(input_text)
        .style(Style::default().fg(Color::Yellow))
        .block(Block::default().title(" Comando [Invio: invia, F1: report day, F2: report week, Esc/Ctrl-C: esci] ").borders(Borders::ALL));
    f.render_widget(input_par, chunks[3]);
}
