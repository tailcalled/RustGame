use std::error::Error;
use tokio::sync::mpsc::{self, Sender};

type BoxErr = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum LobbyCommand {
    StartGame
}

#[derive(Debug)]
pub enum ClientEvent {
    ClientConnect(ClientId),
    ClientGotName(ClientId, String),
    ClientDisconnect(ClientId, Option<BoxErr>),
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ClientId(u64);

pub mod terminal;
pub mod connection;
pub mod host;
pub mod killable;
pub mod world;
pub mod geom;

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
    match choice.as_str() {
        "host" => {
            let (tx, rx) = mpsc::channel(64);
            runtime.spawn(host::host_game(rx, term.clone()));
            runtime.block_on(host_lobby(tx, term));
        }
        value if value.starts_with("join ") => {

        }
        _ =>
            term.println("Command not understood.")?
    }
    Ok(())
}

async fn host_lobby(mut tx: Sender<LobbyCommand>, term: terminal::Terminal) -> Result<(), Box<dyn Error>> {
    term.println("Available commands:")?;
    term.println(" * start -- starts the game")?;
    loop {
        let choice = term.readln("Enter a command.")?;
        match choice.as_str() {
            "start" =>
                drop(tx.send(LobbyCommand::StartGame).await),
            _ =>
                term.println("Invalid command!")?
        }
    }
}
