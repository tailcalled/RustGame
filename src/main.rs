use termion::input::TermRead;
use std::io::{stdin, stdout, Write, Stdin, Stdout};
use std::thread;
use std::sync::mpsc;

enum LobbyCommand { StartGame }

fn main() {
    let mut stdin = stdin();
    let mut stdout = stdout();
    println!("Welcome to RustGame!");
    println!("Please enter your username.");
    print!("> ");
    stdout.flush();
    let username = TermRead::read_line(&mut stdin).unwrap().unwrap_or_else(|| "anonymous".to_string());
    println!("Hello {}!", username);
    println!("Available commands:");
    println!(" * host -- host a game");
    println!(" * join <address> -- join the game hosted at address");
    print!("> ");
    stdout.flush();
    let choice = TermRead::read_line(&mut stdin);
    match choice.unwrap().as_ref().map(String::as_str) {
        None => println!("Goodbye"),
        Some("host") => {
            let (tx, rx) = mpsc::channel();
            thread::spawn(move || {
                host_game(rx);
            });
            host_lobby(tx, stdin, stdout);
        }
        Some(value) if value.starts_with("join ") => {

        }
        Some(_) =>
            println!("Command not understood.")
    }
}

fn host_game(rx: mpsc::Receiver<LobbyCommand>) {

}
fn host_lobby(tx: mpsc::Sender<LobbyCommand>, mut stdin: Stdin, mut stdout: Stdout) {
    println!("Available commands:");
    println!(" * start -- starts the game");
    while (true) {
        print!("> ");
        stdout.flush();
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