use crate::world::{World, EntityId};
use crate::terminal::Scene;
use crate::terminal;
use crate::geom::Vec;
use termion::color::AnsiValue;

pub fn render(world: &World, player_id: &EntityId) -> Box<Scene> {
    let mut scene = Box::new(Scene::default());
    let player = world.entities.get(&player_id).unwrap();
    for sx in 0 .. terminal::SCREEN_W {
        for sy in 0 .. terminal::SCREEN_H {
            let world_pos = player.pos + Vec::new(sx as i32, sy as i32);
            let gridline_x = if world_pos.x % 5 == 0 { 1 } else { 0 };
            let gridline_y = if world_pos.y % 5 == 0 { 1 } else { 0 };
            let back_ch = [' ', '-', '|', '+'][gridline_x+gridline_y*2];
            scene.set_point(sx as i32, sy as i32, back_ch, AnsiValue::rgb(5, 5, 5), AnsiValue::rgb(0, 0, 0));
        }
    }
    for entity in world.entities.values() {
        let screen_pos = entity.pos - player.pos;
        scene.set_point(screen_pos.x, screen_pos.y, '@', AnsiValue::rgb(5, 5, 5), AnsiValue::rgb(0, 0, 0));
    }
    scene
}
