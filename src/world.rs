use rpds::RedBlackTreeMap as Map;
use archery::shared_pointer::kind::{ArcK, SharedPointerKind};
use crate::ClientId;
use crate::geom::*;
use serde::{Serialize, Deserialize};
use std::vec;

#[derive(Debug, Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[derive(Serialize, Deserialize)]
pub struct EntityId(u64);

impl EntityId {
    fn next(self) -> Self {
        EntityId(self.0 + 1)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct World {
    pub entities: Map<EntityId, Entity, ArcK>,
    next_entity_id: EntityId,
}

impl Default for World {
    fn default() -> World {
        World {
            entities : Map::new_with_ptr_kind(),
            next_entity_id : EntityId(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub pos: Vec,
    pub kind: EntityKind,
}
#[derive(Debug, Copy, Clone, Serialize, Deserialize)]
pub enum EntityKind {
    Player(ClientId),
}

impl Entity {
    pub fn is_player(&self, client: ClientId) -> bool {
        match self.kind {
            EntityKind::Player(e_client) => e_client == client,
            //_ => false
        }
    }
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerActionEvent {
    Move(Dir),
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WorldEvent {
    PlayerAction(EntityId, PlayerActionEvent),
    SpawnEntity(EntityId, Entity),
    DeleteEntity(EntityId),
    CreateEntity(Entity),
}

#[derive(Debug)]
pub enum WorldError {
    IllegalEvent,
}

impl World {
    pub fn handle_event(&self, sender: Option<ClientId>, ev: WorldEvent) -> Result<(Self, vec::Vec<(u64, WorldEvent)>), WorldError> {
        let mut w = self.clone();
        let mut evs = vec::Vec::new();
        use WorldEvent::*;
        match (sender, &ev) {
            (None, _) => {}
            (Some(client), PlayerAction(id, _)) =>
                match w.entities.get(&id) {
                    None => Err(WorldError::IllegalEvent)?, // trying to move nonexistent player -- unauthorized, fail
                    Some(e) if e.is_player(client) => {} // authorized -- continue
                    _ => Err(WorldError::IllegalEvent)? // trying to move entity other than self -- unauthorized, fail
                }
            (Some(_), _) => Err(WorldError::IllegalEvent)?
        }
        match ev {
            PlayerAction(id, PlayerActionEvent::Move(dir)) =>
                if w.get_entities_at(w.entities.get(&id).unwrap().pos + dir.to_vec()).next().is_none() {
                    w.entities.modify(id, |player| player.pos += dir.to_vec())
                },
            SpawnEntity(id, entity_data) =>
                w.entities.insert_mut(id, entity_data),
            DeleteEntity(id) =>
                drop(w.entities.remove_mut(&id)),
            CreateEntity(entity_data) => {
                evs.push((0, SpawnEntity(w.next_entity_id, entity_data)));
                w.next_entity_id = w.next_entity_id.next();
            }
        }
        Ok((w, evs))
    }
    pub fn create_player_spawn_event(&self, id: ClientId) -> WorldEvent {
        WorldEvent::CreateEntity(Entity {
            pos: Vec::new(0, 0),
            kind: EntityKind::Player(id),
        })
    }
    pub fn create_player_exit_event(&self, id: ClientId) -> Option<WorldEvent> {
        self.entities.iter()
            .find(|(_eid, entity)| entity.is_player(id))
            .map(|(eid, _)| WorldEvent::DeleteEntity(*eid))
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

impl<K: Ord, V: Clone, P: SharedPointerKind> MapExt for Map<K, V, P> {
    type K = K;
    type V = V;
    fn modify<F: FnOnce(&mut V)>(&mut self, key: K, f: F) {
        let mut value = self.get(&key).unwrap().clone();
        f(&mut value);
        self.insert_mut(key, value);
    }
}
