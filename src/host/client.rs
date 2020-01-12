use std::{io, error::Error};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::sync::mpsc::{self, Receiver, Sender, UnboundedSender};

use futures::future::{join, try_join};

use crate::{FromClientEvent, ClientId};
use crate::terminal::Terminal;
use crate::connection::{split_stream, ConnectionIn, ConnectionOut};
use crate::host::ClientEvent;
use crate::world::World;
use crate::killable::{KillSpawn, KillHandle};

type BoxErr = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum ClientChannel<V> {
    Tokio(Sender<V>),
    Crossbeam(crossbeam::channel::Sender<V>),
}
impl<V> ClientChannel<V> {
    pub fn try_send(&mut self, v: V) -> Result<(), ()> {
        match self {
            ClientChannel::Tokio(chan) => chan.try_send(v).map_err(|_| ()),
            ClientChannel::Crossbeam(chan) => chan.try_send(v).map_err(|_| ()),
        }
    }
}

pub fn local_client(
    name: String,
    term: crate::terminal::Terminal,
    sink: UnboundedSender<ClientEvent>,
) -> (Client, crate::WorldIOHalf) {
    let (netio, worldio) = crate::net_world_channel(term);
    let id = ClientId(0);
    let client = Client {
        client_id: id,
        name,
        addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 0),
        send_events: ClientChannel::Crossbeam(netio.send),
        _handle: KillHandle::empty(),
    };

    let mut recv = netio.recv;
    tokio::spawn(async move {
        while let Some(msg) = recv.recv().await {
            let client_msg = match msg {
                FromClientEvent::Disconnect() =>
                    ClientEvent::Shutdown(),

                FromClientEvent::PlayerEvent(evid, world) =>
                    ClientEvent::WorldEvent(evid, Some(id), world),
            };
            if let Err(_) = sink.send(client_msg) {
                break;
            }
        }
        eprintln!("Local client sender shutdown.");
    });

    (client, worldio)
}

#[derive(Debug)]
pub struct Client {
    pub client_id: ClientId,
    pub name: String,
    pub addr: SocketAddr,
    pub send_events: ClientChannel<(Duration, crate::ToClientEvent)>,
    _handle: KillHandle,
}
impl Client {
    #[must_use]
    pub fn send_event(&mut self, since_start: Duration, ev: crate::ToClientEvent) -> bool {
        if let Err(_) = self.send_events.try_send((since_start, ev)) {
            true
        } else {
            false
        }
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        eprintln!("Dropping client {}.", self.name);
    }
}

pub fn client_received(
    stream: TcpStream,
    addr: SocketAddr,
    sink: UnboundedSender<ClientEvent>,
    client_id: ClientId,
    term: Terminal,
) {
    let (input, output) = split_stream(stream);

    let inner = ClientInner {
        client_id,
        addr,
        sink,
        input,
        output,
        term,
    };

    let (killspawn, handle) = KillSpawn::new();
    killspawn.spawn(start_client_task(inner, handle));
}

struct ClientInner {
    client_id: ClientId,
    addr: SocketAddr,
    sink: UnboundedSender<ClientEvent>,
    input: ConnectionIn,
    output: ConnectionOut,
    term: Terminal,
}

async fn start_client_task(mut inner: ClientInner, handle: KillHandle) {

    let result = try_join(
        inner.input.recv::<String>(),
        inner.output.send::<u64>(&inner.client_id.0),
    ).await;

    let name: String = match result {
        Ok((name, ())) => name,
        Err(err) => {
            let _ = inner.term.println(format!(
                    "Failed to receive name from client: {}",
                    err
            ));
            return;
        },
    };

    let (event_send, event_recv) = mpsc::channel(1024);

    let client = Client {
        client_id: inner.client_id,
        addr: inner.addr,
        send_events: ClientChannel::Tokio(event_send),
        name,
        _handle: handle,
    };

    // Send the client to the main thread, and ask for a copy of the third world
    let (world_send, world_recv) = oneshot::channel();
    match inner.sink.send(ClientEvent::ClientConnected(client, world_send)) {
        Ok(()) => {},
        Err(_) => return, // Main game loop has shut down
    }

    let id = inner.client_id;
    let err_sink = inner.sink.clone();
    match async {
        // receive current state of the third world
        let world = match world_recv.await {
            Ok(world) => world,
            Err(_) => return Ok(()), // game has shut down
        };
        inner.output.send::<World>(&world).await?;

        let (spawn1, handle1) = KillSpawn::new();
        let (spawn2, handle2) = KillSpawn::new();

        let sink = inner.sink.clone();
        let recv = ClientReceiver {
            client_id: inner.client_id,
            sink: inner.sink,
            input: inner.input,
            _kill: handle1,
        };
        let send = ClientSender {
            msgs: event_recv,
            output: inner.output,
            _kill: handle2,
        };

        // By swapping the kill handles, the other is killed if either returns
        let fut1 = spawn1.into_killable(send.handle_output());
        let fut2 = spawn2.into_killable(recv.handle_input());

        match join(fut1, fut2).await {
            (Some(Err(err)), _) => return Err(err),
            (_, Some(Err(err))) => return Err(err),
            (_, _) => {},
        }

        let _ = sink.send(ClientEvent::ClientDisconnect(inner.client_id, None));

        Ok(())
    }.await {
        Ok(()) => {},
        Err(err) => {
            let err: io::Error = err;
            let err = Some(Box::new(err) as BoxErr);
            err_sink.send(ClientEvent::ClientDisconnect(id, err)).unwrap();
        },
    }
}

struct ClientReceiver {
    client_id: ClientId,
    sink: UnboundedSender<ClientEvent>,
    input: ConnectionIn,
    _kill: KillHandle,
}
impl ClientReceiver {
    pub async fn handle_input(mut self) -> io::Result<()> {
        loop {
            let msg: FromClientEvent = self.input.recv().await?;
            let client_msg = match msg {
                FromClientEvent::Disconnect() => return Ok(()),

                FromClientEvent::PlayerEvent(evid, world) =>
                    ClientEvent::WorldEvent(evid, Some(self.client_id), world),
            };
            if let Err(_) = self.sink.send(client_msg) {
                break Ok(());
            }
        }
    }
}

struct ClientSender {
    msgs: Receiver<(Duration, crate::ToClientEvent)>,
    output: ConnectionOut,
    _kill: KillHandle,
}
impl ClientSender {
    pub async fn handle_output(mut self) -> io::Result<()> {
        while let Some(msg) = self.msgs.recv().await {
            self.output.send::<(Duration, crate::ToClientEvent)>(&msg).await?;
        }
        self.output.send(&(Duration::new(0, 0), // FIXME get the time since server start in here somehow
            crate::ToClientEvent::Kick("Client dropped".to_string()))).await?;
        Ok(())
    }
}
