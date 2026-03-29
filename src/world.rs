use noise::{Fbm, Perlin};

use crate::chunk::{BlockType, Chunk, ChunkRefs};
use crate::position::IVec3;
use std::collections::HashMap;
use std::sync::Arc;

pub struct World {
    pub chunks: HashMap<IVec3, Arc<Chunk>>,
    pub render_distance: i32,
    pub fbm: Fbm<Perlin>,
}

impl World {
    pub fn new(render_distance: i32) -> Self {
        Self {
            chunks: HashMap::new(),
            render_distance,
            fbm: Fbm::<Perlin>::new(rand::random()),
        }
    }

    pub fn update_load_area(&mut self, center: IVec3) {
        // Unload chunks outside render distance
        self.chunks.retain(|pos, _| {
            (pos.x - center.x).abs() <= self.render_distance
                && (pos.y - center.y).abs() <= self.render_distance
                && (pos.z - center.z).abs() <= self.render_distance
        });

        // Load new chunks within render distance
        for x in -self.render_distance..=self.render_distance {
            for y in -self.render_distance..=self.render_distance {
                for z in -self.render_distance..=self.render_distance {
                    let pos = IVec3::new(center.x + x, center.y + y, center.z + z);
                    let chunk = match pos.y {
                        ..0 => Chunk::full(BlockType::Dirt),
                        0 => Chunk::new_terain(&self.fbm, pos),
                        _ => Chunk::empty(),
                    };
                    self.chunks.entry(pos).or_insert_with(|| Arc::new(chunk));
                }
            }
        }
    }

    pub fn get_chunk_refs(&self, pos: IVec3) -> Option<ChunkRefs> {
        let mut refs: Vec<Arc<Chunk>> = Vec::with_capacity(27);
        for x in -1..=1 {
            for y in -1..=1 {
                for z in -1..=1 {
                    let neighbor_pos = IVec3::new(pos.x + x, pos.y + y, pos.z + z);
                    if let Some(chunk) = self.chunks.get(&neighbor_pos) {
                        refs.push(chunk.clone());
                    } else {
                        return None; // Cannot mesh if neighbors are missing
                    }
                }
            }
        }

        let refs_array: [Arc<Chunk>; 27] = refs.try_into().ok()?;
        Some(ChunkRefs { refs: refs_array })
    }
}
