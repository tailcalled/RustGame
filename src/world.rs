use rpds::RedBlackTreeMap as Map;
use crate::ClientId;
use crate::geom::*;
use serde::{Serialize, Deserialize};

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct EntityId(u64);

#[derive(Clone, Serialize, Deserialize)]
pub struct World {
    pub entities: Map<EntityId, Entity>,
    next_entity_id: EntityId,
}

impl Default for World {
    fn default() -> World {
        World {
            entities : Map::new(),
            next_entity_id : EntityId(0),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Entity {
    pub pos: Vec,
    pub kind: EntityKind,
}
#[derive(Copy, Clone, Serialize, Deserialize)]
pub enum EntityKind {
    Player(ClientId),
}

#[derive(Debug, Serialize, Deserialize)]
pub enum PlayerActionEvent {
    Move(Dir),
}
#[derive(Debug, Serialize, Deserialize)]
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
        self.entities.iter().filter(move |(_, ent)| ent.pos == pos).map(|(eid, ent)| (*eid, ent))
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
