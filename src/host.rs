use tokio::net::{TcpStream, TcpListener};
use tokio::sync::mpsc::{self, Sender, Receiver};

use futures::future::join;
use futures::stream::StreamExt;

use std::io;
use std::net::SocketAddr;

use crate::LobbyCommand;
use crate::killable::{spawn, KillHandle};
use crate::terminal;
use get_if_addrs::get_if_addrs;

pub mod client;

pub async fn host_game(rx: Receiver<LobbyCommand>, term: terminal::Terminal) {
    match host_game_real(rx, term.clone()).await {
        Err(err) => {
            term.println(format!("Error in host: {}", err));
        },
        Ok(()) => {},
    }
}
async fn host_game_real(lobby: Receiver<LobbyCommand>, term: terminal::Terminal) -> io::Result<()> {
    let host = Host::new().await?;

    fn to_ip(addr: get_if_addrs::IfAddr) -> String {
        use get_if_addrs::IfAddr::*;
        match addr {
            V4(addr) => addr.ip.to_string(),
            V6(addr) => addr.ip.to_string(),
        }
    }

    tokio::task::block_in_place(|| {
        let mut string = String::new();
        get_if_addrs()?.into_iter()
            .filter(|interface| interface.name != "lo")
            .for_each(|interface| {
                string.push_str(&to_ip(interface.addr));
                string.push_str(", ");
            });
        string.pop(); string.pop();
        term.println(format!("Listening on {}", string));
        io::Result::Ok(())
    })?;

    join(
        lobby.for_each(|_cmd| async {
            unimplemented!()
        }),
        host.listen.recv.for_each(|_result| async {
            unimplemented!()
        })
    ).await;

    Ok(())
}

struct Host {
    listen: Acceptor,
}

impl Host {
    pub async fn new() -> io::Result<Self> {
        let listen = TcpListener::bind(("0.0.0.0", 4921)).await?;
        Ok(Host {
            listen: Acceptor::new(listen),
        })
    }
}

struct Acceptor {
    _kill: KillHandle,
    recv: Receiver<io::Result<(TcpStream, SocketAddr)>>,
}
impl Acceptor {
    pub fn new(listen: TcpListener) -> Acceptor {
        let (mut send, recv) = mpsc::channel(1);
        let kill = spawn(async move {
            let send2 = send.clone();
            if let Err(err) = acceptor_thread(listen, send2).await {
                if let Err(err) = send.send(Err(err)).await {
                    panic!("{}", err.0.unwrap_err());
                }
            }
        });

        Acceptor {
            _kill: kill,
            recv,
        }
    }
}

async fn acceptor_thread(
    mut listen: TcpListener,
    mut send: Sender<io::Result<(TcpStream, SocketAddr)>>,
) -> io::Result<()> {
    while let Ok(()) = send.send(listen.accept().await).await { }
    Ok(())
}
