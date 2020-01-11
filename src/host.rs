use std::net::{TcpStream, TcpListener, SocketAddr};
use std::io;
use crossbeam::channel::{self, select, Sender, Receiver};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use crate::LobbyCommand;
use get_if_addrs::get_if_addrs;

pub mod client;

pub fn host_game(rx: Receiver<LobbyCommand>) {
    match host_game_real(rx) {
        Err(err) => {
            println!("Error in host: {}", err);
        },
        Ok(()) => {},
    }
}
fn host_game_real(lobby: Receiver<LobbyCommand>) -> io::Result<()> {
    let host = Host::new()?;
    print!("Hosting a server on ");

    fn to_ip(addr: get_if_addrs::IfAddr) -> String {
        use get_if_addrs::IfAddr::*;
        match addr {
            V4(addr) => addr.ip.to_string(),
            V6(addr) => addr.ip.to_string(),
        }
    }

    let mut first = true;
    let ips = get_if_addrs()?.into_iter()
        .filter(|interface| interface.name != "lo")
        .for_each(|interface| {
            let comma = if first { "" } else { ", " };
            first = false;
            print!("{}{}", comma, to_ip(interface.addr))
        });
    println!();

    loop {
        select! {
            recv(lobby) -> msg => match msg {
                Ok(LobbyCommand::StartGame) => unimplemented!(),
                Err(_) => return Ok(()),
            },
            recv(host.listen.recv) -> msg => match msg {
                Ok(Ok((stream, addr))) => unimplemented!(),
                Ok(Err(err)) => panic!("Error while receiving threads: {}", err),
                Err(_) => panic!("Acceptor thread paniced"),
            },
        }
    }
}

struct Host {
    listen: Acceptor,
}

impl Host {
    pub fn new() -> io::Result<Self> {
        let listen = TcpListener::bind(("0.0.0.0", 4921))?;
        Ok(Host {
            listen: Acceptor::new(listen),
        })
    }
}

struct Acceptor {
    kill: Arc<AtomicBool>,
    recv: Receiver<io::Result<(TcpStream, SocketAddr)>>,
}
impl Acceptor {
    pub fn new(listen: TcpListener) -> Acceptor {
        let kill = Arc::new(AtomicBool::new(false));
        let (send, recv) = channel::bounded(0);

        let kill2 = Arc::clone(&kill);
        std::thread::spawn(move || {
            let send2 = send.clone();
            if let Err(err) = acceptor_thread(listen, send2, kill2) {
                if let Err(err) = send.send(Err(err)) {
                    panic!("{}", err.into_inner().unwrap_err());
                }
            }
        });

        Acceptor {
            kill,
            recv,
        }
    }
}
impl Drop for Acceptor {
    fn drop(&mut self) {
        self.kill.store(true, Ordering::Relaxed);
    }
}

fn acceptor_thread(
    listen: TcpListener,
    send: Sender<io::Result<(TcpStream, SocketAddr)>>,
    kill: Arc<AtomicBool>,
) -> io::Result<()> {
    listen.set_nonblocking(true)?;
    loop {
        match listen.accept() {
            Ok(stream) => if let Err(_) = send.send(Ok(stream)) {
                return Ok(());
            },
            Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                if kill.load(Ordering::Relaxed) {
                    return Ok(());
                }
                std::thread::yield_now();
            },
            Err(e) => return Err(e),
        }
    }
}
