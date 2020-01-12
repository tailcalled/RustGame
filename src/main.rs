use std::error::Error;
use serde::{Serialize, Deserialize};

#[derive(Debug)]
pub enum LobbyCommand {
    StartGame
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct ClientId(u64);

#[derive(Debug, Serialize, Deserialize)]
pub enum FromClientEvent {
    /// Client wants to disconnect.
    Disconnect(),
    /// A player event.
    PlayerEvent(crate::world::WorldEvent),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ToClientEvent {
    NewClientId(ClientId),
    RemoveClientId(ClientId),
    Kick(String),
    WorldEvent(Option<ClientId>, crate::world::WorldEvent),
}

pub struct NetIOHalf {
    pub term: terminal::Terminal,
    pub send: crossbeam::channel::Sender<ToClientEvent>,
    pub recv: tokio::sync::mpsc::UnboundedReceiver<FromClientEvent>,
}

pub struct WorldIOHalf {
    pub term: terminal::Terminal,
    pub send: tokio::sync::mpsc::UnboundedSender<FromClientEvent>,
    pub recv: crossbeam::channel::Receiver<ToClientEvent>,
}

pub fn net_world_channel(term: terminal::Terminal) -> (NetIOHalf, WorldIOHalf) {
    let (to_client_send, to_client_recv) = tokio::sync::mpsc::unbounded_channel();
    let (from_client_send, from_client_recv) = crossbeam::channel::unbounded();
    let term2 = term.clone();
    (
        NetIOHalf { term, send: from_client_send, recv: to_client_recv, },
        WorldIOHalf { term: term2, send: to_client_send, recv: from_client_recv, },
    )
}

/// This will be called in a newly created thread dedicated to the game loop.
pub fn create_game_loop(io: WorldIOHalf, world: world::World, my_id: ClientId) {
    world_handler::handle_world(io, world, my_id)
}

pub mod terminal;
pub mod connection;
pub mod host;
pub mod killable;
pub mod world;
pub mod geom;
pub mod renderer;
pub mod world_handler;

fn main() -> Result<(), Box<dyn Error>>{
    let mut runtime = tokio::runtime::Runtime::new()?;

    let term = terminal::Terminal::new();
    term.println("Welcome to RustGame!")?;
    let username = term.readln("Please enter your username.")?;
    term.println(format!("Hello {}!", username))?;
    term.println("Available commands:")?;
    term.println(" * host -- host a game")?;
    term.println(" * join <address> -- join the game hosted at address")?;
    let choice = term.readln("Please pick an option to start the game.")?;
    match choice.as_str().trim() {
        "host" => {
            runtime.block_on(host::host_game(term.clone()));
        }
        value if value.starts_with("join ") => {

        }
        _ =>
            term.println("Command not understood.")?
    }
    Ok(())
}
