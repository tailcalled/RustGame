use png_pong;
use pix::Rgba8;
use std::collections::HashMap;
use crate::world::{Tile, TileMap, GroundKind, TerrainKind, Chunk, CHUNK_SIZE};
use crate::geom::Vec;

pub fn load_level() -> TileMap {
    let data = std::fs::read("data/level.png").expect("Failed to open level file.");
    let data = std::io::Cursor::new(data);
    let decoder = png_pong::FrameDecoder::<_, Rgba8>::new(data);
    let mut raster = decoder.last().expect("No frames in png").expect("PNG parsing error").raster;
    let mut anchor_colors = HashMap::new();
    anchor_colors.insert((0, 0, 0, 255), "origin".to_string());
    let anchor_colors = anchor_colors;
    let mut anchors = HashMap::new();
    for px in 0 .. raster.width() {
        for py in 0 .. raster.height() {
            match anchor_colors.get(&raster.pixel(px, py).to_tuple()) {
                None => {}
                Some(name) => {
                    anchors.insert(name, Vec::new(px as i32, py as i32));
                    raster.set_pixel(px, py, raster.pixel(px+1, py+1));
                }
            }
        }
    }
    let origin = *anchors.get(&"origin".to_string()).unwrap();
    let left = (origin.x + (CHUNK_SIZE - 1) as i32) / CHUNK_SIZE as i32;
    let top = (origin.y + (CHUNK_SIZE - 1) as i32) / CHUNK_SIZE as i32;
    let right = (raster.width() as i32 - origin.x) / CHUNK_SIZE as i32;
    let bottom = (raster.height() as i32 - origin.y) / CHUNK_SIZE as i32;
    let mut tile_types = HashMap::new();
    tile_types.insert((0, 255, 0, 255), Tile { ground : Some(GroundKind::Grass), terrain : None});
    tile_types.insert((0, 0, 255, 255), Tile { ground : Some(GroundKind::Water), terrain : None});
    tile_types.insert((0, 127, 0, 255), Tile { ground : Some(GroundKind::Grass), terrain : Some(TerrainKind::Tree)});
    tile_types.insert((127, 51, 0, 255), Tile { ground : Some(GroundKind::Rock), terrain : Some(TerrainKind::Cliff)});
    let tile_types = tile_types;
    let mut tile_map = TileMap::new();
    for cx in -left .. right {
        for cy in -top .. bottom {
            let mut chunk : Chunk = Default::default();
            let mut non_empty = false;
            for px in 0 .. CHUNK_SIZE as i32 {
                for py in 0 .. CHUNK_SIZE as i32 {
                    let world_pos = Vec::new((cx * CHUNK_SIZE as i32)+px, (cy * CHUNK_SIZE as i32)+py);
                    let screen_pos = world_pos + origin;
                    if screen_pos.x < 0 || screen_pos.y < 0 || screen_pos.x > raster.width() as i32 || screen_pos.y > raster.height() as i32 {
                        continue;
                    }
                    let pixel = raster.pixel(screen_pos.x as u32, screen_pos.y as u32).to_tuple();
                    if pixel.3 == 0 {
                        continue;
                    }
                    match tile_types.get(&pixel) {
                        None => panic!("Missing tile type! {:?}", pixel),
                        Some(tile) => {
                            chunk[px as usize][py as usize] = tile.clone();
                            non_empty = true;
                        }
                    }
                }
            }
            if non_empty {
                println!("Including chunk! {} {}", cx, cy);
                tile_map.set_chunk(cx, cy, chunk);
            }
        }
    }
    tile_map
}

trait RgbaExt {
    fn to_tuple(self) -> (u8, u8, u8, u8);
}
impl RgbaExt for Rgba8 {
    fn to_tuple(self) -> (u8, u8, u8, u8) {
        (self.red().into(), self.green().into(), self.blue().into(), pix::Alpha::value(&self.alpha()).into())
    }
}