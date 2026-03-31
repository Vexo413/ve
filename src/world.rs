use ahash::{AHashMap, AHashSet};
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};

use crate::chunk::{Chunk, ChunkRefs};
use crate::constants::*;
use crate::position::IVec3;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

pub enum WorldRequest {
    GenerateChunks { x: i32, z: i32, ys: Vec<i32> },
}

pub enum WorldResponse {
    ChunksGenerated { chunks: Vec<(IVec3, Arc<Chunk>)> },
}

pub struct World {
    pub chunks: AHashMap<IVec3, Arc<Chunk>>,
    pub render_distance: i32,
    request_tx: Sender<WorldRequest>,
    response_rx: Receiver<WorldResponse>,
    pending_chunks: AHashSet<IVec3>,
}

impl World {
    pub fn new(render_distance: i32) -> Self {
        let (request_tx, request_rx) = channel::<WorldRequest>();
        let (response_tx, response_rx) = channel::<WorldResponse>();

        thread::spawn(move || {
            let mut noise = FastNoiseLite::new();
            noise.set_noise_type(Some(NoiseType::OpenSimplex2));
            noise.set_fractal_type(Some(FractalType::Ridged));
            noise.set_fractal_octaves(Some(3));
            noise.set_fractal_lacunarity(Some(2.0));
            noise.set_fractal_gain(Some(0.5));

            while let Ok(request) = request_rx.recv() {
                match request {
                    WorldRequest::GenerateChunks { x, z, ys } => {
                        let mut heights = [0i32; CHUNK_SIZE2_U];
                        for lx in 0..CHUNK_SIZE {
                            for lz in 0..CHUNK_SIZE {
                                let noise_x = x as f32 * CHUNK_SIZE as f32 + lx as f32;
                                let noise_z = z as f32 * CHUNK_SIZE as f32 + lz as f32;
                                let h = noise.get_noise_2d(noise_x, noise_z).powi(2) * 32.0;
                                heights[lx as usize * CHUNK_SIZE_U + lz as usize] = h as i32;
                            }
                        }

                        let mut generated_chunks = Vec::new();
                        for y in ys {
                            let pos = IVec3::new(x, y, z);
                            let chunk = Chunk::new_terrain(pos, &heights);
                            generated_chunks.push((pos, Arc::new(chunk)));
                        }

                        let _ = response_tx.send(WorldResponse::ChunksGenerated {
                            chunks: generated_chunks,
                        });
                    }
                }
            }
        });

        Self {
            chunks: AHashMap::new(),
            render_distance,
            request_tx,
            response_rx,
            pending_chunks: AHashSet::new(),
        }
    }

    pub fn process_responses(&mut self) {
        while let Ok(response) = self.response_rx.try_recv() {
            match response {
                WorldResponse::ChunksGenerated { chunks } => {
                    for (pos, chunk) in chunks {
                        self.pending_chunks.remove(&pos);
                        self.chunks.insert(pos, chunk);
                    }
                }
            }
        }
    }

    pub fn update_load_area(&mut self, center: IVec3) {
        let render_distance = self.render_distance + 1;
        // Unload chunks outside render distance
        self.chunks.retain(|pos, _| {
            (pos.x - center.x).abs() <= render_distance
                && (pos.y - center.y).abs() <= render_distance
                && (pos.z - center.z).abs() <= render_distance
        });
        // Also clean up pending chunks that are now out of range
        self.pending_chunks.retain(|pos| {
            (pos.x - center.x).abs() <= render_distance
                && (pos.y - center.y).abs() <= render_distance
                && (pos.z - center.z).abs() <= render_distance
        });

        // Load new chunks within render distance
        for x_off in -render_distance..=render_distance {
            for z_off in -render_distance..=render_distance {
                let x = center.x + x_off;
                let z = center.z + z_off;

                let mut missing_ys = Vec::new();
                for y_off in -render_distance..=render_distance {
                    let y = center.y + y_off;
                    let pos = IVec3::new(x, y, z);
                    if !self.chunks.contains_key(&pos) && !self.pending_chunks.contains(&pos) {
                        missing_ys.push(y);
                        self.pending_chunks.insert(pos);
                    }
                }

                if !missing_ys.is_empty() {
                    let _ = self.request_tx.send(WorldRequest::GenerateChunks {
                        x,
                        z,
                        ys: missing_ys,
                    });
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
