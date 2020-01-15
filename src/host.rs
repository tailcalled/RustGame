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

use crate::{ClientId, ToClientEvent, EventId, gen_event_id};
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
    WorldEvent(EventId, Option<ClientId>, crate::world::WorldEvent),
    Shutdown(),
}

pub async fn host_game(term: Terminal, local_name: String) {
    match host_game_real(term.clone(), local_name).await {
        Err(err) => {
            eprintln!("Error in host: {}", err);
        },
        Ok(()) => {},
    }
}
async fn host_game_real(
    term: Terminal,
    local_name: String,
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
    let mut next_client_id = 1;

    let (sink, mut client_events) = mpsc::unbounded_channel();

    // spawn local client
    let (local_client, worldio) = self::client::local_client(
        local_name, term.clone(), sink.clone()
    );
    let local_id = local_client.client_id;

    let (local_world_send, local_world_recv) = oneshot::channel();
    tokio::spawn(async move {
        let world = local_world_recv.await.unwrap();
        std::thread::Builder::new().name("game loop".to_string())
            .spawn(move || {
                crate::create_game_loop(worldio, world, local_id);
            }).unwrap();
    });
    sink.send(ClientEvent::ClientConnected(local_client, local_world_send)).unwrap();

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

    let server_start_time = Instant::now();
    while let Some(event) = client_events.recv().await {
        eprintln!("Event: {:?}", event);
        match event {
            ClientEvent::ClientConnected(client, world_send) => {
                // Broadcast new client id
                host.broadcast(Instant::now() - server_start_time,
                    ToClientEvent::NewClientId(client.client_id));

                // Add to list of clients.
                let id = client.client_id;
                host.add_client(client);

                // Send world to new client.
                let world = host.third_world.clone();
                let _ = world_send.send(world);

                // Create world event for entity.
                let ev = host.third_world.create_player_spawn_event(id);
                let ev = ClientEvent::WorldEvent(gen_event_id(), None, ev);

                // This send wont fail -- the receiver is up there in the while loop.
                sink.send(ev).unwrap();
            },
            ClientEvent::ClientDisconnect(id, Some(err)) => {
                let removed = host.clients.remove(&id).unwrap();

                let ev = host.third_world.create_player_exit_event(removed.client_id);
                if let Some(ev) = ev {
                    let ev = ClientEvent::WorldEvent(gen_event_id(), None, ev);
                    sink.send(ev).unwrap();
                }

                host.broadcast(Instant::now() - server_start_time,
                    ToClientEvent::RemoveClientId(id));

                let _ = term.println(format!(
                    "Disconnected {}: {}",
                    removed.name,
                    err
                ));
            },
            ClientEvent::ClientDisconnect(id, None) => {
                let removed = host.clients.remove(&id).unwrap();

                host.broadcast(Instant::now() - server_start_time, ToClientEvent::RemoveClientId(id));

                let _ = term.println(format!(
                    "Disconnected {}.",
                    removed.name,
                ));
            },
            ClientEvent::WorldEvent(evid, id, event) =>
                match host.third_world.handle_event(id, event.clone()) {
                    Ok((next_world, mut events)) => {
                        let now = Instant::now();
                        host.broadcast(now - server_start_time, ToClientEvent::WorldEvent(evid, id, event));
                        host.third_world = next_world;

                        // Sort events by decreasing time
                        events.sort_by_key(|(time, _)| *time);
                        events.reverse();
                        // Send events with time zero
                        while let Some((0, _)) = events.last() {
                            let ev = events.pop().unwrap().1;
                            let _ = sink.send(ClientEvent::WorldEvent(gen_event_id(), None, ev));
                        }
                        // Send the remaining using timers
                        if events.len() > 0 {
                            let sink = sink.clone();
                            tokio::spawn(async move {
                                for (time, ev) in events.into_iter().rev() {
                                    delay_until(now + Duration::from_millis(time)).await;
                                    match sink.send(ClientEvent::WorldEvent(gen_event_id(), None, ev)) {
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
                            let _ = client.send_event(Instant::now() - server_start_time, ToClientEvent::Kick(
                                    format!("Third world error: {:?}", error)));
                        } else {
                            term.println(
                                format!("Third world error: {:?}", error)).unwrap();
                            return Ok(());
                        }
                    },
                },
            ClientEvent::Shutdown() => {
                host.broadcast(Instant::now() - server_start_time, ToClientEvent::Kick("Server shutting down.".into()));
                return Ok(());
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
    pub fn broadcast(&mut self, since_start: Duration, msg: ToClientEvent) {
        let mut remove = Vec::new();
        for client in self.clients.values_mut() {
            if !client.send_event(since_start, msg.clone()) {
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
            third_world: Default::default(),
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
