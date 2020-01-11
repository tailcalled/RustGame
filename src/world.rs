use rpds::{RedBlackTreeMap as Map};
use crate::ClientId;
use crate::geom::*;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct EntityId(usize);

#[derive(Clone)]
pub struct World {
    entities: Map<EntityId, Entity>,
    next_entity_id: EntityId,
}
#[derive(Clone)]
pub struct Entity {
    pos: Vec,
    kind: EntityKind,
}
#[derive(Copy, Clone)]
pub enum EntityKind {
    Player(ClientId),
}

pub enum PlayerActionEvent {
    Move(Dir),
}
pub enum WorldEvent {
    PlayerAction(EntityId, PlayerActionEvent),
}

pub enum WorldError {
    IllegalEvent,
}

impl World {
    pub fn handle_event(&self, sender: Option<ClientId>, ev: WorldEvent) -> Result<Self, WorldError> {
        let mut w = self.clone();
        use WorldEvent::*;
        match (sender, &ev) {
            (None, _) => {}
            (Some(client), PlayerAction(id, _)) =>
                match w.entities.get(&id) {
                    None => Err(WorldError::IllegalEvent)?, // trying to move nonexistent player -- unauthorized, fail
                    Some(e) =>
                        match e.kind {
                            EntityKind::Player(entity_client) if entity_client == client => {} // authorized -- continue
                            _ => Err(WorldError::IllegalEvent)? // trying to move entity other than self -- unauthorized, fail
                        }
                }
        }
        match ev {
            PlayerAction(id, PlayerActionEvent::Move(dir)) =>
                if w.get_entities_at(w.entities.get(&id).unwrap().pos + dir.to_vec()).next().is_none() {
                    w.entities.modify(id, |player| player.pos += dir.to_vec())
                }
        }
        Ok(w)
    }
    pub fn get_entities_at(&self, pos: Vec) -> impl Iterator<Item=(EntityId, &Entity)> {
        self.entities.iter().filter(move |(eid, ent)| ent.pos == pos).map(|(eid, ent)| (*eid, ent))
    }
}

trait MapExt {
    type K;
    type V: Clone;
    fn modify<F: FnOnce(&mut Self::V)>(&mut self, key: Self::K, f: F);
}

impl<K: Ord, V: Clone> MapExt for Map<K, V> {
    type K = K;
    type V = V;
    fn modify<F: FnOnce(&mut V)>(&mut self, key: K, f: F) {
        let mut value = self.get(&key).unwrap().clone();
        f(&mut value);
        self.insert_mut(key, value);
    }
}