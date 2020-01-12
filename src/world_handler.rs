use crossbeam::channel;
use crossbeam::channel::{select};
use crate::terminal;
use crate::world::*;
use crate::geom::*;
use crate::{WorldIOHalf, ClientId, ToClientEvent};
use crate::renderer;
use std::thread;

pub fn handle_world(world_io: WorldIOHalf, start_world: World, me: ClientId) {
    let (uitx, uirx) = channel::unbounded();
    let mut uitx = Some(uitx);
    let mut self_entity = None;
    let mut agreed_world = start_world.clone();
    let mut speculative_world = start_world;
    select! {
        recv(uirx) -> _ => // speculative evaluation
            {}, // TODO
        recv(world_io.recv) -> msg => { // definitive evaluation
            let msg = msg.unwrap();
            match (&mut uitx, &msg) {
                (None, _) => {}
                (uitx, ToClientEvent::WorldEvent(None, WorldEvent::SpawnEntity(id, entity)))
                    if entity.is_player(me) => {
                    self_entity = Some(id.clone());
                    start_ui_input(*id, uitx.take().unwrap(), world_io.term.clone());
                }
                _ => {}
            }
            match msg {
                ToClientEvent::NewClientId(_) => {}
                ToClientEvent::RemoveClientId(_) => {}
                ToClientEvent::Kick(reason) => {
                    world_io.term.println(format!("You have been kicked: {}", reason)).unwrap();
                    return;
                }
                ToClientEvent::WorldEvent(owner, ev) => {
                    let (new_world, pending_events) = agreed_world.handle_event(owner, ev).unwrap();
                    agreed_world = new_world;
                    drop(pending_events); // for future reference, use these for speculative evaluation
                    speculative_world = agreed_world.clone();
                    match &self_entity {
                        None => {}
                        Some(entity) => render_world(&speculative_world, entity, &world_io.term)
                    }
                }
            }
        }
    }
}

fn render_world(world: &World, player: &EntityId, term: &terminal::Terminal) {
    let scene = renderer::render(world, player);
    term.draw_scene(scene).unwrap();
}

fn start_ui_input(entity: EntityId, uitx: channel::Sender<WorldEvent>, term: terminal::Terminal) {
    thread::spawn (move || {
        loop {
            term.readln("Press enter to move left.").unwrap();
            uitx.send(WorldEvent::PlayerAction(entity, PlayerActionEvent::Move(Dir::left()))).unwrap();
        }
    });
}
