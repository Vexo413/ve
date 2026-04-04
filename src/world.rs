use crate::chunk::{Chunk, ChunkRefs};
use crate::constants::*;
use crate::io::{IORequest, IOResponse, load_chunk, save_chunk};
use crate::position::IVec3;
use fastnoise_lite::{FastNoiseLite, FractalType, NoiseType};
use hashbrown::{HashMap, HashSet};
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
    pub chunks: HashMap<IVec3, Arc<Chunk>>,
    pub render_distance: i32,
    gen_request_sender: Sender<WorldRequest>,
    gen_response_receiver: Receiver<WorldResponse>,
    io_request_sender: Sender<IORequest>,
    io_response_receiver: Receiver<IOResponse>,
    pending_chunks: HashSet<IVec3>,
    loading_chunks: HashSet<IVec3>,
}

impl World {
    pub fn new(render_distance: i32) -> Self {
        let (gen_request_sender, gen_request_receiver) = channel::<WorldRequest>();
        let (gen_response_sender, gen_response_receiver) = channel::<WorldResponse>();
        let (io_request_sender, io_request_receiver) = channel::<IORequest>();
        let (io_response_sender, io_response_receiver) = channel::<IOResponse>();

        // Generator thread
        thread::spawn(move || {
            let mut noise = FastNoiseLite::new();
            noise.set_noise_type(Some(NoiseType::OpenSimplex2));
            noise.set_fractal_type(Some(FractalType::Ridged));
            noise.set_fractal_octaves(Some(9));
            noise.set_fractal_lacunarity(Some(2.0));
            noise.set_fractal_gain(Some(0.5));

            while let Ok(request) = gen_request_receiver.recv() {
                match request {
                    WorldRequest::GenerateChunks { x, z, ys } => {
                        let mut heights = [0i32; CHUNK_SIZE2_U];
                        for lx in 0..CHUNK_SIZE {
                            for lz in 0..CHUNK_SIZE {
                                let noise_x = x as f32 * CHUNK_SIZE as f32 + lx as f32;
                                let noise_z = z as f32 * CHUNK_SIZE as f32 + lz as f32;
                                let h = noise.get_noise_2d(noise_x / 8.0, noise_z / 8.0).powi(2)
                                    * 128.0;
                                heights[lx as usize * CHUNK_SIZE_U + lz as usize] = h as i32;
                            }
                        }

                        let mut generated_chunks = Vec::new();
                        for y in ys {
                            let pos = IVec3::new(x, y, z);
                            let chunk = Chunk::new_terrain(pos, &heights);
                            let arc_chunk = Arc::new(chunk);
                            generated_chunks.push((pos, arc_chunk));
                        }

                        let _ = gen_response_sender.send(WorldResponse::ChunksGenerated {
                            chunks: generated_chunks,
                        });
                    }
                }
            }
        });

        // IO thread
        thread::spawn(move || {
            while let Ok(request) = io_request_receiver.recv() {
                match request {
                    IORequest::LoadChunk(position) => {
                        let chunk = match load_chunk(position) {
                            Ok(Some(c)) => Some(Arc::new(c)),
                            _ => None,
                        };
                        let _ = io_response_sender.send(IOResponse::ChunkLoaded(position, chunk));
                    }
                    IORequest::SaveChunk { position, chunk } => {
                        let _ = save_chunk(position, &chunk);
                        let _ = io_response_sender.send(IOResponse::ChunkSaved(position));
                    }
                }
            }
        });

        Self {
            chunks: HashMap::new(),
            render_distance,
            gen_request_sender,
            gen_response_receiver,
            io_request_sender,
            io_response_receiver,
            pending_chunks: HashSet::new(),
            loading_chunks: HashSet::new(),
        }
    }

    pub fn process_responses(&mut self) {
        // Process generator responses
        while let Ok(response) = self.gen_response_receiver.try_recv() {
            match response {
                WorldResponse::ChunksGenerated { chunks } => {
                    for (position, chunk) in chunks {
                        if self.pending_chunks.remove(&position) {
                            // Save newly generated chunk to disk
                            let _ = self.io_request_sender.send(IORequest::SaveChunk {
                                position,
                                chunk: chunk.clone(),
                            });
                            self.chunks.insert(position, chunk);
                        }
                    }
                }
            }
        }

        // Process IO responses
        while let Ok(response) = self.io_response_receiver.try_recv() {
            match response {
                IOResponse::ChunkLoaded(pos, chunk) => {
                    self.loading_chunks.remove(&pos);
                    if let Some(chunk) = chunk {
                        if self.pending_chunks.remove(&pos) {
                            self.chunks.insert(pos, chunk);
                        }
                    } else {
                        // Not found on disk, generate it
                        if self.pending_chunks.contains(&pos) {
                            let _ = self.gen_request_sender.send(WorldRequest::GenerateChunks {
                                x: pos.x,
                                z: pos.z,
                                ys: vec![pos.y],
                            });
                        }
                    }
                }
                IOResponse::ChunkSaved(_) => {}
            }
        }
    }

    pub fn clear_chunk(&mut self, position: IVec3) {
        if let Some(chunk) = self.chunks.get_mut(&position) {
            let mut new_chunk = (**chunk).clone();
            new_chunk.voxels = [0u8; CHUNK_SIZE3_U];
            let chunk = Arc::new(new_chunk);
            self.chunks.insert(position, chunk.clone());
            let _ = self
                .io_request_sender
                .send(IORequest::SaveChunk { position, chunk });
        }
    }

    pub fn update_load_area(&mut self, center: IVec3) {
        let render_distance = self.render_distance + 1;
        // Unload chunks outside render distance
        self.chunks.retain(|position, chunk| {
            let keep = (position.x - center.x).abs() <= render_distance
                && (position.y - center.y).abs() <= render_distance
                && (position.z - center.z).abs() <= render_distance;

            if !keep {
                // Save chunk on unload
                let _ = self.io_request_sender.send(IORequest::SaveChunk {
                    position: *position,
                    chunk: chunk.clone(),
                });
            }
            keep
        });
        // Also clean up pending chunks that are now out of range
        self.pending_chunks.retain(|position| {
            (position.x - center.x).abs() <= render_distance
                && (position.y - center.y).abs() <= render_distance
                && (position.z - center.z).abs() <= render_distance
        });
        self.loading_chunks.retain(|position| {
            (position.x - center.x).abs() <= render_distance
                && (position.y - center.y).abs() <= render_distance
                && (position.z - center.z).abs() <= render_distance
        });

        // Load new chunks within render distance
        for x_off in -render_distance..=render_distance {
            let x = center.x + x_off;
            for z_off in -render_distance..=render_distance {
                let z = center.z + z_off;
                for y_off in -render_distance..=render_distance {
                    let y = center.y + y_off;
                    let pos = IVec3::new(x, y, z);
                    if !self.chunks.contains_key(&pos) && !self.pending_chunks.contains(&pos) {
                        self.pending_chunks.insert(pos);
                        self.loading_chunks.insert(pos);
                        let _ = self.io_request_sender.send(IORequest::LoadChunk(pos));
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
