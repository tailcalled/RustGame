use std::{io, error};
use std::net::{TcpStream, SocketAddr};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use crossbeam::channel::{self, Sender, Receiver};
use crate::connection::Connection;
use crate::{ClientEvent, ClientId};

pub struct Client {
    addr: SocketAddr,
    kill: Arc<AtomicBool>,
    client_id: ClientId,
}

impl Client {
    pub fn new(
        stream: TcpStream,
        addr: SocketAddr,
        sink: Sender<ClientEvent>,
        client_id: ClientId,
    ) -> io::Result<Self> {
        let stream = Connection::new(stream)?;
        let kill = Arc::new(AtomicBool::new(false));
        sink_fail.send(ClientConnect(client_id)).unwrap();
        std::thread::spawn(move || {
            let sink_fail = sink.clone();
            let c = ClientInner {
                stream,
                sink,
                kill: kill2,
                id: client_id,
            };
            use crate::ClientEvent::ClientDisconnect;
            if let Err(err) = handle_client(c) {
                let _ = sink_fail.send(ClientDisconnect(client_id, Some(err)));
            } else {
                let _ = sink_fail.send(ClientDisconnect(client_id, None));
            }
        });
        Ok(Client {
            addr,
            kill,
            client_id,
        })
    }
}
impl Drop for Client {
    fn drop(&mut self) {
        self.kill.store(true, Ordering::Relaxed);
    }
}

struct ClientInner {
    stream: Connection,
    sink: Sender<ClientEvent>,
    kill: Arc<AtomicBool>,
    id: ClientId,
}

fn handle_client(c: ClientInner) -> Result<(), Box<dyn Error>> {
    let name: String = self.stream.recv()?;
    c.sink.send(ClientEvent::ClientGotName(self.id, name))?;
}
