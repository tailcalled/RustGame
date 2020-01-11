use std::{io, error::Error};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::net::SocketAddr;
use tokio::net::TcpStream;
use tokio::sync::mpsc::{self, Sender, Receiver};
use crate::connection::Connection;
use crate::{ClientEvent, ClientId};

type BoxErr = Box<dyn Error + Send + Sync + 'static>;

pub struct Client {
    addr: SocketAddr,
    client_id: ClientId,
    _kill: crate::killable::KillHandle,
}

impl Client {
    pub async fn new(
        stream: TcpStream,
        addr: SocketAddr,
        mut sink: Sender<ClientEvent>,
        client_id: ClientId,
    ) -> io::Result<Self> {
        let stream = Connection::new(stream)?;
        let _ = sink.send(ClientEvent::ClientConnect(client_id)).await;
        let kill = crate::killable::spawn(async move {
            let mut sink_fail = sink.clone();
            let c = ClientInner {
                stream,
                sink,
                id: client_id,
            };
            use crate::ClientEvent::ClientDisconnect;
            if let Err(err) = handle_client(c).await {
                let _ = sink_fail.send(ClientDisconnect(client_id, Some(err))).await;
            } else {
                let _ = sink_fail.send(ClientDisconnect(client_id, None)).await;
            }
        });
        Ok(Client {
            addr,
            client_id,
            _kill: kill,
        })
    }
}

struct ClientInner {
    stream: Connection,
    sink: Sender<ClientEvent>,
    id: ClientId,
}

async fn handle_client(mut c: ClientInner) -> Result<(), BoxErr> {
    let name: String = c.stream.recv().await?;
    c.sink.send(ClientEvent::ClientGotName(c.id, name)).await?;
    Ok(())
}
