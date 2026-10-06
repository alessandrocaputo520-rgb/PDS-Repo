//! Invio di messaggi server -> utenti con canali per connessione.
use common::protocol::ServerMessage;
use std::collections::HashMap;
use tokio::sync::mpsc::UnboundedSender;

pub struct Session {
    pub username: String,
    pub sender: UnboundedSender<ServerMessage>,
}
#[derive(Default)]
pub struct Hub {
    sessions: HashMap<i64, Session>,
}
impl Hub {
    pub fn insert(
        &mut self,
        id: i64,
        username: String,
        sender: UnboundedSender<ServerMessage>,
    ) -> Result<(), String> {
        if self.sessions.contains_key(&id) {
            return Err("Questo account è già connesso".into());
        }
        self.sessions.insert(id, Session { username, sender });
        Ok(())
    }
    pub fn remove(&mut self, id: i64) {
        self.sessions.remove(&id);
    }
    pub fn broadcast(&mut self, from: &str, content: &str) -> usize {
        let msg = ServerMessage::BroadcastMessage {
            from: from.into(),
            content: content.into(),
        };
        self.sessions
            .values()
            .filter(|s| s.sender.send(msg.clone()).is_ok())
            .count()
    }
    pub fn direct(&self, id: i64, from: &str, content: &str) -> bool {
        self.sessions
            .get(&id)
            .map(|s| {
                s.sender
                    .send(ServerMessage::DirectMessage {
                        from: from.into(),
                        content: content.into(),
                    })
                    .is_ok()
            })
            .unwrap_or(false)
    }
    pub fn online(&self) -> Vec<(i64, String)> {
        let mut values: Vec<_> = self
            .sessions
            .iter()
            .map(|(id, s)| (*id, s.username.clone()))
            .collect();
        values.sort_by(|a, b| a.1.cmp(&b.1));
        values
    }
}
