use tokio::net::{TcpStream, TcpListener};
use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::oneshot;
use futures::stream::StreamExt;
use tokio::time::{Instant, delay_until};

use std::time::Duration;
use std::io;
use std::error::Error;
use std::net::SocketAddr;
use std::collections::HashMap;

use crate::{ClientId, ToClientEvent};
use crate::killable::{spawn, KillHandle};
use crate::terminal::Terminal;
use crate::world::World;
use get_if_addrs::get_if_addrs;

pub mod client;
use self::client::Client;

type BoxErr = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum ClientEvent {
    ClientConnected(Client, oneshot::Sender<World>),
    ClientDisconnect(ClientId, Option<BoxErr>),
    WorldEvent(Option<ClientId>, crate::world::WorldEvent),
}

pub async fn host_game(term: Terminal) {
    match host_game_real(term.clone()).await {
        Err(err) => {
            let _ = term.println(format!("Error in host: {}", err));
        },
        Ok(()) => {},
    }
}
async fn host_game_real(
    term: Terminal
) -> io::Result<()> {
    let accept = Acceptor::new().await?;
    fn to_ip(addr: get_if_addrs::IfAddr) -> String {
        use get_if_addrs::IfAddr::*;
        match addr {
            V4(addr) => addr.ip.to_string(),
            V6(addr) => addr.ip.to_string(),
        }
    }

    let mut string = String::new();
    get_if_addrs()?.into_iter()
        .filter(|interface| interface.name != "lo")
        .for_each(|interface| {
            string.push_str(&to_ip(interface.addr));
            string.push_str(", ");
        });
    string.pop(); string.pop();
    let _ = term.println(format!("Listening on {}", string));

    let mut host = Host::new();
    let mut next_client_id = 0;

    let (sink, mut client_events) = mpsc::unbounded_channel();

    let accept_sink = sink.clone();
    let term_accept = term.clone();
    tokio::spawn(accept.recv.for_each(move |result| {
        let accept_sink = accept_sink.clone();
        let term_accept = term_accept.clone();
        let id = ClientId(next_client_id);
        next_client_id += 1;
        async move {
            let (stream, addr) = match result {
                Ok(ok) => ok,
                Err(err) => {
                    let _ = term_accept.println(
                        format!("Failed to accept connection: {}", err));
                    return;
                },
            };
            self::client::client_received(
                stream,
                addr,
                accept_sink,
                id,
                term_accept.clone(),
            );
        }
    }));

    while let Some(event) = client_events.recv().await {
        match event {
            ClientEvent::ClientConnected(client, world_send) => {
                // Broadcast new client id
                host.broadcast(ToClientEvent::NewClientId(client.client_id));

                // Add to list of clients.
                let id = client.client_id;
                host.add_client(client);

                // Send world to new client.
                let world = host.third_world.clone();
                let _ = world_send.send(world);

                // Create world event for entity.
                let ev = World::create_player_spawn_event(id);
                let ev = ClientEvent::WorldEvent(None, ev);

                // This send wont fail -- the receiver is up there in the while loop.
                sink.send(ev).unwrap();
            },
            ClientEvent::ClientDisconnect(id, Some(err)) => {
                host.clients.remove(&id);

                host.broadcast(ToClientEvent::RemoveClientId(id));

                let _ = term.println(format!(
                    "Disconnected {}: {}",
                    host.clients.get(&id).unwrap().name,
                    err
                ));
            },
            ClientEvent::ClientDisconnect(id, None) => {
                host.clients.remove(&id);

                host.broadcast(ToClientEvent::RemoveClientId(id));

                let _ = term.println(format!(
                    "Disconnected {}.",
                    host.clients.get(&id).unwrap().name,
                ));
            },
            ClientEvent::WorldEvent(id, event) =>
                match host.third_world.handle_event(id, event.clone()) {
                    Ok((next_world, mut events)) => {
                        host.broadcast(ToClientEvent::WorldEvent(id, event));
                        host.third_world = next_world;

                        let now = Instant::now();

                        // Sort events by decreasing time
                        events.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
                        // Send events with time zero
                        while let Some((0, _)) = events.last() {
                            let ev = events.pop().unwrap().1;
                            let _ = sink.send(ClientEvent::WorldEvent(None, ev));
                        }
                        // Send the remaining using timers
                        if events.len() > 0 {
                            let sink = sink.clone();
                            tokio::spawn(async move {
                                for (time, ev) in events.into_iter().rev() {
                                    delay_until(now + Duration::from_millis(time)).await;
                                    match sink.send(ClientEvent::WorldEvent(None, ev)) {
                                        Ok(()) => {},
                                        Err(_) => return,
                                    }
                                }
                            });
                        }
                    },
                    Err(error) => {
                        if let Some(id) = id {
                            let mut client = host.clients.remove(&id).unwrap();
                            let _ = client.send_event(ToClientEvent::Kick(
                                    format!("Third world error: {:?}", error)));
                        } else {
                            term.println(
                                format!("Third world error: {:?}", error)).unwrap();
                            return Ok(());
                        }
                    },
                },
        }
    }

    Ok(())
}

struct Host {
    clients: HashMap<ClientId, client::Client>,
    third_world: World,
}
impl Host {
    pub fn broadcast(&mut self, msg: ToClientEvent) {
        let mut remove = Vec::new();
        for client in self.clients.values_mut() {
            if !client.send_event(msg.clone()) {
                remove.push(client.client_id);
            }
        }
        for id in remove {
            self.clients.remove(&id);
        }
    }
}

impl Host {
    pub fn new() -> Self {
        #[allow(unreachable_code)]
        Host {
            clients: HashMap::new(),
            third_world: unimplemented!(),
        }
    }
    pub fn add_client(&mut self, client: client::Client) {
        self.clients.insert(client.client_id, client);
    }
}

struct Acceptor {
    _kill: KillHandle,
    recv: Receiver<io::Result<(TcpStream, SocketAddr)>>,
}
impl Acceptor {
    pub async fn new() -> io::Result<Acceptor> {
        let listen = TcpListener::bind(("0.0.0.0", 4921)).await?;
        let (mut send, recv) = mpsc::channel(1);
        let kill = spawn(async move {
            let send2 = send.clone();
            if let Err(err) = acceptor_thread(listen, send2).await {
                if let Err(err) = send.send(Err(err)).await {
                    panic!("{}", err.0.unwrap_err());
                }
            }
        });

        Ok(Acceptor {
            _kill: kill,
            recv,
        })
    }
}

async fn acceptor_thread(
    mut listen: TcpListener,
    mut send: Sender<io::Result<(TcpStream, SocketAddr)>>,
) -> io::Result<()> {
    while let Ok(()) = send.send(listen.accept().await).await { }
    Ok(())
}
