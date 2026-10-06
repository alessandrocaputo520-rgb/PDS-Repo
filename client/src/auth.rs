use crate::network::{self, Writer};
use common::protocol::{recv_msg, ClientMessage, ServerMessage};
use std::io;
use tokio::{
    io::{BufReader, Lines, Stdin},
    net::tcp::OwnedReadHalf,
};

pub async fn login(
    input: &mut Lines<BufReader<Stdin>>,
    reader: &mut OwnedReadHalf,
    writer: &Writer,
) -> io::Result<String> {
    loop {
        println!("Scegli [login] o [register] (oppure quit):");
        let Some(choice) = input.next_line().await? else {
            return Err(io::Error::new(
                io::ErrorKind::UnexpectedEof,
                "input terminato",
            ));
        };
        if choice.trim().eq_ignore_ascii_case("quit") {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "uscita"));
        }
        if !["login", "register"].contains(&choice.trim()) {
            continue;
        }
        println!("Username:");
        let username = input
            .next_line()
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "input terminato"))?;
        println!("Password (visibile nella console):");
        let password = input
            .next_line()
            .await?
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "input terminato"))?;
        let msg = if choice.trim() == "login" {
            ClientMessage::Login { username, password }
        } else {
            ClientMessage::Register { username, password }
        };
        network::send(writer, &msg).await?;
        match recv_msg::<_, ServerMessage>(reader).await? {
            ServerMessage::AuthSuccess { username, .. } => {
                println!("Accesso riuscito: {username}");
                return Ok(username);
            }
            ServerMessage::AuthFailure { reason } => eprintln!("Autenticazione: {reason}"),
            other => eprintln!("Risposta inattesa: {other:?}"),
        }
    }
}
