use termion::input::TermRead;
use std::io::{stdin, stdout, Write, Stdin, Stdout};
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
struct ClientId(u64);

pub mod connection;
pub mod host;

fn main() {
    let mut stdin = stdin();
    let mut stdout = stdout();
    println!("Welcome to RustGame!");
    println!("Please enter your username.");
    print!("> ");
    stdout.flush().unwrap();
    let username = TermRead::read_line(&mut stdin).unwrap().unwrap_or_else(|| "anonymous".to_string());
    println!("Hello {}!", username);
    println!("Available commands:");
    println!(" * host -- host a game");
    println!(" * join <address> -- join the game hosted at address");
    print!("> ");
    stdout.flush().unwrap();
    let choice = TermRead::read_line(&mut stdin);
    match choice.unwrap().as_ref().map(String::as_str) {
        None => println!("Goodbye"),
        Some("host") => {
            let (tx, rx) = channel::unbounded();
            thread::spawn(move || {
                host::host_game(rx);
            });
            host_lobby(tx, stdin, stdout);
        }
        Some(value) if value.starts_with("join ") => {

        }
        Some(_) =>
            println!("Command not understood.")
    }
}

fn host_lobby(tx: channel::Sender<LobbyCommand>, mut stdin: Stdin, mut stdout: Stdout) {
    println!("Available commands:");
    println!(" * start -- starts the game");
    loop {
        print!("> ");
        stdout.flush().unwrap();
        let choice = TermRead::read_line(&mut stdin);
        match choice.unwrap().as_ref().map(String::as_str) {
            None => {},
            Some("start") =>
                tx.send(LobbyCommand::StartGame).unwrap(),
            Some(_) =>
                println!("Invalid command!")
        }
    }
}
