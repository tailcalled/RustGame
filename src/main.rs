use std::error::Error;
use serde::{Serialize, Deserialize};

#[derive(Debug)]
pub enum LobbyCommand {
    StartGame
}

#[derive(Serialize, Deserialize)]
pub enum WorldEvent {}
#[derive(Serialize, Deserialize)]
pub enum ReceiveEvent {
    Disconnect(),
    WorldEvent(WorldEvent),
}

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

