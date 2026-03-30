use ahash::AHashMap;
use noise::{Fbm, MultiFractal, NoiseFn, Perlin};

use crate::chunk::{BlockType, Chunk, ChunkRefs};
use crate::constants::*;
use crate::position::IVec3;
use std::sync::Arc;

pub struct World {
    pub chunks: AHashMap<IVec3, Arc<Chunk>>,
    pub render_distance: i32,
    pub fbm: Fbm<Perlin>,
}

impl World {
    pub fn new(render_distance: i32) -> Self {
        Self {
            chunks: AHashMap::new(),
            render_distance,
            fbm: Fbm::<Perlin>::new(rand::random()).set_octaves(9),
        }
    }

    pub fn update_load_area(&mut self, center: IVec3) {
        let render_distance = self.render_distance + 1; // Rendering chunks requires neigboring, adding one makes sure the chunks exist
        // Unload chunks outside render distance
        self.chunks.retain(|pos, _| {
            (pos.x - center.x).abs() <= render_distance
                && (pos.y - center.y).abs() <= render_distance
                && (pos.z - center.z).abs() <= render_distance
        });

        // Load new chunks within render distance
        for x in -render_distance..=render_distance {
            for z in -render_distance..=render_distance {
                // Calculate height map for this (x, z) column of chunks
                let mut heights = [0i32; CHUNK_SIZE2_U];
                for lx in 0..CHUNK_SIZE {
                    for lz in 0..CHUNK_SIZE {
                        let noise_x: f64 = (center.x + x) as f64 * CHUNK_SIZE as f64 + lx as f64;
                        let noise_z: f64 = (center.z + z) as f64 * CHUNK_SIZE as f64 + lz as f64;
                        // let h =
                        //     (self.fbm.get([noise_x / 128.0, noise_z / 128.0]) - 0.5).powi(4) * 64.0;
                        let mut h = (noise_x / 64.0).sin() * (noise_z / 64.0).cos() * 64.0;
                        if rand::random() {
                            h += 1.0;
                        }

                        // dbg!(h);
                        heights[lx as usize * CHUNK_SIZE_U + lz as usize] = h as i32;
                    }
                }

                for y in -render_distance..=render_distance {
                    let pos = IVec3::new(center.x + x, center.y + y, center.z + z);
                    if !self.chunks.contains_key(&pos) {
                        let chunk = match pos.y {
                            // ..-3 => Chunk::full(BlockType::Dirt),
                            -3..=1 => Chunk::new_terrain(pos, &heights),
                            _ => Chunk::new_terrain(pos, &heights),
                            // _ => Chunk::empty(),
                        };
                        self.chunks.insert(pos, Arc::new(chunk));
                    }
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
