use crossbeam::channel;
use crossbeam::channel::{select};
use crate::terminal;
use crate::world::*;
use crate::geom::*;
use crate::{WorldIOHalf, ClientId, ToClientEvent, FromClientEvent, gen_event_id};
use crate::renderer;
use std::thread;
use std::vec;
use std::time::{Instant, Duration};

pub fn handle_world(world_io: WorldIOHalf, start_world: World, me: ClientId) {
    let (uitx, uirx) = channel::unbounded::<WorldEvent>();
    let mut uitx = Some(uitx);
    let mut self_entity = None;
    let mut agreed_world = start_world.clone();
    let mut speculative_world = start_world;
    let mut awaiting_events = vec::Vec::new();
    let start_time = Instant::now();
    let mut est_delta = Duration::new(0, 0);
    loop {
        select! {
            recv(uirx) -> msg => { // speculative evaluation, TODO
                    let msg = msg.unwrap();
                    let id = gen_event_id();
                    let ev = FromClientEvent::PlayerEvent(id, msg.clone());
                    world_io.send.send(ev).unwrap();
                    awaiting_events.push((Instant::now() - start_time, id, Some(me), msg.clone()));
                    speculative_world = speculative_world.handle_event(Some(me), msg).unwrap().0; // TODO: save speculative auto events
                },
            recv(world_io.recv) -> msg => { // definitive evaluation
                let msg = msg.unwrap();
                match (&mut uitx, &msg) {
                    (None, _) => {}
                    (uitx, (_, ToClientEvent::WorldEvent(_, None, WorldEvent::SpawnEntity(id, entity))))
                        if entity.is_player(me) => {
                        self_entity = Some(id.clone());
                        start_ui_input(*id, uitx.take().unwrap(), world_io.term.clone());
                    }
                    _ => {}
                }
                match (&self_entity, &msg) {
                    (None, _) => {}
                    (Some(self_id), (_, ToClientEvent::WorldEvent(_, _, WorldEvent::DeleteEntity(id)))) if self_id == id => {
                        world_io.send.send(FromClientEvent::Disconnect());
                        world_io.term.println("You died!");
                        thread::sleep(Duration::from_millis(100));
                        return;
                    }
                    _ => {}
                }
                match msg {
                    (_, ToClientEvent::NewClientId(_)) => {}
                    (_, ToClientEvent::RemoveClientId(_)) => {}
                    (_, ToClientEvent::Kick(reason)) => {
                        world_io.term.println(format!("You have been kicked: {}", reason)).unwrap();
                        return;
                    }
                    (time, ToClientEvent::WorldEvent(evid, owner, ev)) => {
                        let (new_world, mut pending_events) = agreed_world.handle_event(owner, ev).unwrap();
                        agreed_world = new_world;
                        pending_events.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
                        speculative_world = agreed_world.clone();
                        if owner == Some(me) {
                            awaiting_events = awaiting_events.into_iter().skip_while(|(_, id, _, _)| *id != evid).skip_while(|(_, id, _, _)| *id == evid).collect();
                        }
                        else {
                            // FIXME this seems dangerous...
                            // what if there are multiple events happening at the same time?
                            awaiting_events = awaiting_events.into_iter().skip_while(|(offset, _, _, _)| *offset + est_delta < time).collect();
                        }
                        for (offset, _, owner, ev) in awaiting_events.iter().take_while(|(offset, _, _, _)| *offset < Instant::now() - start_time) {
                            speculative_world = speculative_world.handle_event(*owner, ev.clone()).unwrap().0; // TODO: save speculative auto events
                        }
                        match &self_entity {
                            None => {}
                            Some(entity) => render_world(&speculative_world, entity, &world_io.term)
                        }
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
        term.println("Use WASD to move.");
        loop {
            let ev = term.get_ev().unwrap();
            use termion::event::*;
            match ev {
                Event::Key(Key::Char(ch)) if is_wasd(ch) =>
                    uitx.send(WorldEvent::PlayerAction(entity, PlayerActionEvent::Move(wasd_to_dir(ch)))).unwrap(),
                Event::Key(Key::Char(ch)) if is_wasd(ch.to_ascii_lowercase()) =>
                    uitx.send(WorldEvent::PlayerAction(entity, PlayerActionEvent::Attack(wasd_to_dir(ch.to_ascii_lowercase())))).unwrap(),
                _ => ()
            }
        }
    });
}

fn is_wasd(ch: char) -> bool {
    ch == 'w' || ch == 'a' || ch == 's' || ch == 'd'
}
fn wasd_to_dir(ch: char) -> Dir {
    match ch {
        'w' => Dir::up(),
        'a' => Dir::left(),
        's' => Dir::down(),
        'd' => Dir::right(),
        _ => unreachable!(),
    }
}