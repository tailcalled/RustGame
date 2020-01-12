use tokio::net::TcpStream;
use futures::future::try_join;

use std::io;
use std::error::Error;
use std::time::Duration;

use crate::{ClientId, FromClientEvent, ToClientEvent};
use crate::world::World;
use crate::terminal::Terminal;
use crate::connection::split_stream;

type BoxErr = Box<dyn Error + Send + Sync + 'static>;

#[derive(Debug)]
pub enum ServerEvent {
    LostConnection(BoxErr),
    Event(ToClientEvent),
}

pub async fn join_game(term: Terminal, ip: String, name: String) {
    match join_game_real(term.clone(), ip, name).await {
        Err(err) => {
            let _ = term.println(format!("Error in join: {}", err));
        },
        Ok(()) => {},
    }
}
async fn join_game_real(
    term: Terminal,
    ip: String,
    name: String,
) -> io::Result<()> {

    let (mut input, mut output) = match TcpStream::connect((ip.as_str(), 4921)).await {
        Ok(conn) => split_stream(conn),
        Err(err) => {
            term.println(format!("Failed to connect: {}", err)).unwrap();
            return Ok(());
        },
    };

    let ((), id) = try_join(
        output.send(&name),
        input.recv()
    ).await?;
    let id = ClientId(id);
    term.println("Successfully connected. Receiving world.").unwrap();

    let world: World = input.recv().await?;

    let (netio, worldio) = crate::net_world_channel(term);

    std::thread::Builder::new().name("game loop".to_string())
        .spawn(move || {
            crate::create_game_loop(worldio, world, id);
        }).unwrap();

    let send = netio.send;
    let mut recv = netio.recv;

    tokio::spawn(async move {
        while let Some(msg) = recv.recv().await {
            output.send::<FromClientEvent>(&msg).await?;
            if let FromClientEvent::Disconnect() = msg {
                return Ok(());
            }
        }
        Result::<(), io::Error>::Ok(())
    });

    loop {
        let msg = input.recv::<(Duration, ToClientEvent)>().await?;
        if let Err(_) = send.send(msg) {
            break;
        }
    }

    Ok(())
}

