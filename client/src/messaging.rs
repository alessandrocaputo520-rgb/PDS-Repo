use common::protocol::ServerMessage;

pub fn display(msg: ServerMessage) {
    match msg {
        ServerMessage::PositionAck { state } => println!("[GPS] stato: {state}"),
        ServerMessage::BroadcastMessage { from, content } => {
            println!("[BROADCAST] {from}: {content}")
        }
        ServerMessage::DirectMessage { from, content } => println!("[MESSAGGIO] {from}: {content}"),
        ServerMessage::AuthFailure { reason } => eprintln!("Autenticazione: {reason}"),
        ServerMessage::AuthSuccess { username, .. } => println!("Connesso come {username}"),
        ServerMessage::Error { message } => eprintln!("[ERRORE] {message}"),
    }
}
