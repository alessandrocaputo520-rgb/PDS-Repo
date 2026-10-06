//! Trasporto TCP length-prefixed bincode definito nel crate comune.
use common::protocol::{send_msg, ClientMessage};
use std::{io, sync::Arc};
use tokio::{
    net::{
        tcp::{OwnedReadHalf, OwnedWriteHalf},
        TcpStream,
    },
    sync::Mutex,
};

pub type Writer = Arc<Mutex<OwnedWriteHalf>>;
pub async fn connect(addr: &str) -> io::Result<(OwnedReadHalf, Writer)> {
    let stream = TcpStream::connect(addr).await?;
    let (reader, writer) = stream.into_split();
    Ok((reader, Arc::new(Mutex::new(writer))))
}
pub async fn send(writer: &Writer, msg: &ClientMessage) -> io::Result<()> {
    send_msg(&mut *writer.lock().await, msg).await
}
