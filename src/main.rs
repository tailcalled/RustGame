use std::error::Error;
use std::thread;
use crossbeam::channel;

pub enum LobbyCommand {
    StartGame
}

pub enum ClientEvent {
    ClientConnect(ClientId),
    ClientGotName(ClientId, String),
    ClientDisconnect(ClientId, Option<Box<dyn Error>>),
}

#[derive(Copy, Clone)]
pub struct ClientId(u64);

pub mod terminal;
//pub mod connection;
//pub mod host;

fn main() -> Result<(), Box<dyn Error>>{
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
            let (tx, rx) = channel::unbounded();
            thread::spawn(move || {
                //host::host_game(rx);
            });
            host_lobby(tx, term)?;
        }
        value if value.starts_with("join ") => {

        }
        _ =>
            term.println("Command not understood.")?
    }
    Ok(())
}

fn host_lobby(tx: channel::Sender<LobbyCommand>, term: terminal::Terminal) -> Result<(), Box<dyn Error>> {
    term.println("Available commands:")?;
    term.println(" * start -- starts the game")?;
    loop {
        let choice = term.readln("Enter a command.")?;
        match choice.as_str() {
            "start" =>
                tx.send(LobbyCommand::StartGame).unwrap(),
            _ =>
                term.println("Invalid command!")?
        }
    }
}
