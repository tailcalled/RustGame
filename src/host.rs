use tokio::net::{TcpStream, TcpListener};
use tokio::sync::mpsc::{self, Sender, Receiver};

use futures::stream::StreamExt;

use std::io;
use std::error::Error;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::killable::{spawn, KillHandle};
use crate::terminal::Terminal;
use get_if_addrs::get_if_addrs;

pub mod client;
use self::client::Client;

type BoxErr = Box<dyn Error + Send + Sync + 'static>;
#[derive(Debug)]
pub enum ClientEvent {
    ClientConnected(Client),
    ClientDisconnect(ClientId, Option<BoxErr>),
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub struct ClientId(u64);

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
    let next_client_id = Arc::new(AtomicU64::new(0));

    let (sink, mut client_events) = mpsc::channel(128);

    let term_accept = term.clone();
    tokio::spawn(accept.recv.for_each(move |result| {
        let sink = sink.clone();
        let term_accept = term_accept.clone();
        let next_client_id = next_client_id.clone();
        async move {
            let (stream, addr) = match result {
                Ok(ok) => ok,
                Err(err) => {
                    let _ = term_accept.println(
                        format!("Failed to accept connection: {}", err));
                    return;
                },
            };
            let id = ClientId(next_client_id.fetch_add(1, Ordering::Relaxed));
            if let Err(err) = self::client::client_received(
                stream,
                addr,
                sink,
                id,
                term_accept.clone(),
            ) {
                let _ = term_accept.println(
                    format!("Failed while receiving client: {}", err));
            }
        }
    }));

    while let Some(event) = client_events.recv().await {
        match event {
            ClientEvent::ClientConnected(client) => host.add_client(client),
            ClientEvent::ClientDisconnect(id, Some(err)) => {
                let _ = term.println(format!(
                    "Disconnected {}: {}",
                    host.clients.get(&id).unwrap().name,
                    err
                ));
            },
            ClientEvent::ClientDisconnect(id, None) => {
                let _ = term.println(format!(
                    "Disconnected {}.",
                    host.clients.get(&id).unwrap().name,
                ));
            },
        }
    }

    Ok(())
}

struct Host {
    clients: HashMap<ClientId, client::Client>,
}

impl Host {
    pub fn new() -> Self {
        Host {
            clients: HashMap::new(),
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
