use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Sender, Receiver};
use futures::stream::StreamExt;

use std::io;
use std::error::Error;
use std::net::SocketAddr;
use std::collections::HashMap;

use crate::{ClientId, ToClientEvent, NetIOHalf};
use crate::killable::{spawn, KillHandle};
use crate::terminal::Terminal;
use crate::connection::Connection;

type BoxErr = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum ServerEvent {
    LostConnection(BoxErr),
    Event(ToClientEvent),
}

pub async fn join_game(term: Terminal, ip: String, name: String) {
    match join_game_real(term.clone(), ip, name).await {
        Err(err) => {
            let _ = term.println(format!("Error in join: {}", err));
        },
        Ok(()) => {},
    }
}
async fn join_game_real(
    term: Terminal,
    ip: String,
    name: String,
) -> io::Result<()> {

    let conn = match TcpStream::connect((ip.as_str(), 4921)).await {
        Ok(conn) => Connection::new(conn),
        Err(err) => {
            term.println(format!("Failed to connect: {}", err));
            return Ok(());
        },
    };

    conn.send(name).await?;
    let id: u64 = conn.recv().await?;
    io.println("Successfully connected.");

    Ok(())
}

