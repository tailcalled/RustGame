use crate::world::{World, EntityId, Tile, GroundKind, TerrainKind};
use crate::terminal::Scene;
use crate::terminal;
use crate::geom::Vec;
use termion::color::AnsiValue;

pub fn render(world: &World, player_id: &EntityId) -> Box<Scene> {
    let mut scene = Box::new(Scene::default());
    let player = world.entities.get(&player_id).unwrap();
    let offset = player.pos - Vec::new(terminal::SCREEN_W as i32/2, terminal::SCREEN_H as i32/2);
    for sx in 0 .. terminal::SCREEN_W {
        for sy in 0 .. terminal::SCREEN_H {
            let world_pos = offset + Vec::new(sx as i32, sy as i32);
            let gridline_x = if world_pos.x % 32 == 0 { 1 } else { 0 };
            let gridline_y = if world_pos.y % 32 == 0 { 1 } else { 0 };
            let back_ch = [' ', '|', '-', '+'][gridline_x+gridline_y*2];
            scene.set_point(sx as i32, sy as i32, back_ch, AnsiValue::rgb(5, 5, 5), Some(AnsiValue::rgb(0, 0, 0)));
            let tile = world.tiles.get(world_pos);
            render_tile(tile, &mut scene, sx as i32, sy as i32);
        }
    }
    for entity in world.entities.values() {
        let screen_pos = entity.pos - offset;
        scene.set_point(screen_pos.x, screen_pos.y, '@', AnsiValue::rgb(5, 5, 5), None);
    }
    let (hp, maxhp) = player.hp.unwrap();
    scene.write(format!("HP: {}/{}", hp, maxhp), 0, 0);
    scene.write(format!("Inventory: {}", player.inventory.as_ref().unwrap().count()), 0, 1);
    scene
}

fn render_tile(tile: Tile, scene: &mut Scene, sx: i32, sy: i32) {
    match tile.ground {
        None => {}
        Some(GroundKind::Grass) => {
            scene.set_point(sx, sy, '"', AnsiValue::rgb(0, 2, 0), Some(AnsiValue::rgb(0, 3, 0)));
        }
        Some(GroundKind::Water) => {
            scene.set_point(sx, sy, '~', AnsiValue::rgb(3, 3, 5), Some(AnsiValue::rgb(0, 0, 5)));
        }
        Some(GroundKind::Rock) => {
            scene.set_point(sx, sy, '.', AnsiValue::rgb(3, 1, 0), Some(AnsiValue::rgb(0, 0, 0)));
        }
    }
    match tile.terrain {
        None => {}
        Some(TerrainKind::Tree) => {
            scene.set_point(sx, sy, 'Δ', AnsiValue::rgb(0, 0, 0), None);
        }
        Some(TerrainKind::Cliff) => {
            scene.set_point(sx, sy, '#', AnsiValue::rgb(0, 0, 0), Some(AnsiValue::rgb(3, 1, 0)));
        }
    }
}