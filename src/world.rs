use rpds::RedBlackTreeMap as Map;
use archery::shared_pointer::kind::{ArcK, SharedPointerKind};
use crate::ClientId;
use crate::geom::*;
use serde::{Serialize, Deserialize};
use std::vec;
use crate::level_loader;

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
    pub tiles : TileMap,
}

impl Default for World {
    fn default() -> World {
        World {
            entities : Map::new_with_ptr_kind(),
            next_entity_id : EntityId(0),
            tiles : level_loader::load_level(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub pos: Vec,
    pub kind: EntityKind,
    pub hp: Option<(i64, i64)>
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

pub const CHUNK_SIZE: usize = 32;

pub type Chunk = [[Tile; CHUNK_SIZE]; CHUNK_SIZE];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TileMap {
    chunks: Map<(i32, i32), Chunk, ArcK>
}

impl TileMap {
    pub fn new() -> TileMap {
        TileMap {
            chunks : Map::new_with_ptr_kind()
        }
    }
    pub fn set_chunk(&mut self, cx: i32, cy: i32, ch: Chunk) {
        self.chunks.insert_mut((cx, cy), ch);
    }
    fn conv_pos(pos: Vec) -> (i32, i32, usize, usize) {
        let cx = pos.x.div_euclid(CHUNK_SIZE as i32);
        let cy = pos.y.div_euclid(CHUNK_SIZE as i32);
        let px = pos.x.rem_euclid(CHUNK_SIZE as i32) as usize;
        let py = pos.y.rem_euclid(CHUNK_SIZE as i32) as usize;
        (cx, cy, px, py)
    }
    pub fn get(&self, pos: Vec) -> Tile {
        let (cx, cy, px, py) = TileMap::conv_pos(pos);
        match self.chunks.get(&(cx, cy)) {
            Some(chunk) => chunk[px][py].clone(),
            None => Default::default(),
        }
    }
    pub fn set(&mut self, pos: Vec, tile: Tile) {
        let (cx, cy, px, py) = TileMap::conv_pos(pos);
        let mut chunk = match self.chunks.get(&(cx, cy)) {
            Some(chunk) => chunk.clone(),
            None => Default::default(),
        };
        chunk[px][py] = tile;
        self.chunks.insert_mut((cx, cy), chunk);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Tile {
    pub ground: Option<GroundKind>,
    pub terrain: Option<TerrainKind>,
}

impl Default for Tile {
    fn default() -> Tile {
        Tile {
            ground : None,
            terrain : None,
        }
    }
}
impl Tile {
    fn is_free(self) -> bool {
        self.ground != Some(GroundKind::Water) && self.terrain == None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum GroundKind {
    Grass, Rock, Water
}
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub enum TerrainKind {
    Tree, Cliff
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PlayerActionEvent {
    Move(Dir),
    Attack(Dir),
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
                if w.is_free(w.entities.get(&id).unwrap().pos + dir.to_vec()) {
                    w.entities.modify(id, |player| player.pos += dir.to_vec())
                },
            PlayerAction(id, PlayerActionEvent::Attack(dir)) => {
                let attack_pos = w.entities.get(&id).unwrap().pos + dir.to_vec();
                for id in w.get_entities_at(attack_pos).map(|(id, _)| id).collect::<vec::Vec<_>>() {
                    w.hurt(&mut evs, id, 1);
                }
                w.break_tile(&mut evs, attack_pos);
            }
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
            hp: Some((10, 10)),
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
    fn hurt(&mut self, evs: &mut vec::Vec<(u64, WorldEvent)>, id: EntityId, dmg: i64) {
        match self.entities.get(&id).unwrap().hp {
            None => {}
            Some((mut hp, max)) => {
                hp -= dmg;
                self.entities.modify(id, |ent| ent.hp = Some((hp, max)));
                if hp <= 0 {
                    evs.push((0, WorldEvent::DeleteEntity(id)));
                }
            }
        }
    }
    fn is_free(&self, pos: Vec) -> bool {
        self.tiles.get(pos).is_free() && self.get_entities_at(pos).next().is_none()
    }
    fn break_tile(&mut self, evs: &mut vec::Vec<(u64, WorldEvent)>, pos: Vec) {
        let mut tile = self.tiles.get(pos);
        match tile.terrain {
            None => {}
            Some(TerrainKind::Tree) => {
                tile.terrain = None
            }
            Some(_) => {}
        }
        self.tiles.set(pos, tile);
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
