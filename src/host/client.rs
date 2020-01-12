use std::{io, error::Error};
use std::net::SocketAddr;

use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::sync::mpsc::Sender;

use crate::{FromClientEvent, ClientId};
use crate::terminal::Terminal;
use crate::connection::Connection;
use crate::host::ClientEvent;
use crate::killable::{spawn, KillHandle};

type BoxErr = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug)]
pub struct Client {
    pub client_id: ClientId,
    pub name: String,
    pub addr: SocketAddr,
    _handle: KillHandle,
}

pub fn client_received(
    stream: TcpStream,
    addr: SocketAddr,
    sink: Sender<ClientEvent>,
    client_id: ClientId,
    term: Terminal,
) -> io::Result<()> {
    let stream = Connection::new(stream)?;

    let inner = ClientInner {
        client_id,
        addr,
        sink,
        stream,
        term,
    };
    let (send, recv) = oneshot::channel();
    let handle = spawn(start_client_task(inner, recv));
    send.send(handle).unwrap();
    Ok(())
}

struct ClientInner {
    client_id: ClientId,
    addr: SocketAddr,
    sink: Sender<ClientEvent>,
    stream: Connection,
    term: Terminal,
}

type Recv = oneshot::Receiver<KillHandle>;
async fn start_client_task(mut inner: ClientInner, recv: Recv) {
    let handle = recv.await.unwrap();

    let name = match inner.stream.recv().await {
        Ok(name) => name,
        Err(err) => {
            let _ = inner.term.println(format!(
                    "Failed to receive name from client: {}",
                    err
            ));
            return;
        },
    };
    let client = Client {
        client_id: inner.client_id,
        addr: inner.addr,
        name,
        _handle: handle,
    };
    match inner.sink.send(ClientEvent::ClientConnected(client)).await {
        Ok(()) => {},
        Err(_) => return,
    }

    match async {
        loop {
            let msg: FromClientEvent = inner.stream.recv().await?;
            let client_msg = match msg {
                FromClientEvent::Disconnect() =>
                    ClientEvent::ClientDisconnect(inner.client_id, None),

                FromClientEvent::PlayerEvent(world) =>
                    ClientEvent::WorldEvent(world),
            };
            if let Err(_) = inner.sink.send(client_msg).await {
                break Ok(());
            }
        }
    }.await {
        Ok(()) => {},
        Err(err) => {
            let err: io::Error = err;
            let err = Some(Box::new(err) as BoxErr);
            inner.sink.send(ClientEvent::ClientDisconnect(
                    inner.client_id, err)).await.unwrap();
        },
    }
}

