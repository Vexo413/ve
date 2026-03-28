mod constants;
mod position;
#[cfg(test)]
mod test;
mod wgpu;

use constants::*;
use std::{collections::HashMap, sync::Arc};

use crate::position::UVec3;

fn main() {
    wgpu::start();
}
// 0: Not used
// H: Height in greedy meshing
// T: Block type
// F: Facing direction
// Z: Z position in chunk
// Y: Y position in chunk
// X: X position in chunk
// 00HHHHHTTTTTTTFFFZZZZZYYYYYZZZZZ
#[derive(Debug)]
pub struct Instance(u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum BlockType {
    Empty,
    Dirt,
}

impl BlockType {
    pub fn is_solid(&self) -> bool {
        !matches!(self, BlockType::Empty)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct Quad {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

enum FaceDirection {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

fn greedy_mesh(layer: &[u32; 32]) -> Vec<Quad> {
    let mut quads = Vec::new();
    let mut layer = layer.to_owned();
    let mut x = 0;
    while x < CHUNK_SIZE {
        let mut y = 0;
        while y < CHUNK_SIZE {
            let temp_value = (layer[x as usize] >> y).trailing_zeros();
            y += temp_value;
            if y >= CHUNK_SIZE {
                break;
            }
            let h = (layer[x as usize] >> y).trailing_ones();
            let h_mask_bottom = u32::checked_shl(1, h).map_or(!0, |v| v - 1);
            let h_mask = h_mask_bottom << y;
            let mut w = 1;
            while x + w < CHUNK_SIZE {
                if layer[(x + w) as usize] & h_mask == h_mask {
                    layer[(x + w) as usize] = layer[(x + w) as usize] & !h_mask;
                    w += 1;
                } else {
                    break;
                }
            }
            quads.push(Quad { x, y, w, h });
            y += h;
        }
        x += 1;
    }
    quads
}

#[derive(Copy, Clone)]
struct Chunk {
    voxels: [BlockType; CHUNK_SIZE3_U],
}

impl Chunk {
    pub fn get(&self, x: u32, y: u32, z: u32) -> BlockType {
        self.voxels[x as usize * CHUNK_SIZE2_U + y as usize * CHUNK_SIZE_U + z as usize]
    }
}

struct ChunkRefs {
    refs: [Arc<Chunk>; 27],
}

impl ChunkRefs {
    fn get_from_chunk(&self, x: u32, y: u32, z: u32, chunk_index: u32) -> BlockType {
        self.refs[chunk_index as usize].get(x, y, z)
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> BlockType {
        // x = (-32..=-1)-(0..=31)-(32..=63)
        let x = (x + 32) as u32;
        // x = (0..=31)-(32..=63)-(64..=95)

        let y = (y + 32) as u32;
        let z = (z + 32) as u32;
        let (x_chunk, x) = ((x / 32), (x % 32));
        let (y_chunk, y) = ((y / 32), (y % 32));
        let (z_chunk, z) = ((z / 32), (z % 32));

        let chunk_index = x_chunk * 9 + y_chunk * 3 + z_chunk;
        self.get_from_chunk(x, y, z, chunk_index)
    }
    pub fn get_only_self(&self, x: u32, y: u32, z: u32) -> BlockType {
        // 13 is the index of the center chunk
        self.get_from_chunk(x, y, z, 13)
    }
}

fn mesh(chunk_refs: ChunkRefs) -> Vec<Instance> {
    // 3 Axis
    // CHUNK_SIZE3 worth of binary data
    // so [u32; CHUNK_SIZE2]
    let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
    // 3 Axis
    // 2 Directions
    // CHUNK_SIZE3 worth of binary data
    // so [u32; CHUNK_SIZE2]
    let mut culled_mask = [0u32; CHUNK_SIZE2_U * 3 * 2];

    for x in 0..CHUNK_SIZE {
        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                if chunk_refs.get_only_self(x, y, z).is_solid() {
                    // More significant or to the left is the positive direction

                    // (y, z): x
                    occupied[/*0 * CHUNK_SIZE2_U +*/ y as usize *CHUNK_SIZE_U + z as usize] |=
                        1u32 << x;
                    // (z, x): y
                    occupied[1 * CHUNK_SIZE2_U + z as usize * CHUNK_SIZE_U + x as usize] |=
                        1u32 << y;
                    // (x, y): z
                    occupied[2 * CHUNK_SIZE2_U + x as usize * CHUNK_SIZE_U + y as usize] |=
                        1u32 << z;
                }
            }
        }
    }

    for axis in 0..3 {
        for i in 0..CHUNK_SIZE_U {
            for j in 0..CHUNK_SIZE_U {
                let column = occupied[axis * CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j];

                culled_mask[axis * 2 * CHUNK_SIZE2_U + /*0 * CHUNK_SIZE2_U +*/ i * CHUNK_SIZE_U + j] =
                    column & !(column >> 1);
                culled_mask[axis * 2 * CHUNK_SIZE2_U + 1 * CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j] =
                    column & !(column << 1);
            }
        }
    }

    let mut data: [HashMap<u32, HashMap<u32, [u32; 32]>>; 6];
    data = [
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    ];

    for axis in 0..6 {
        for i in 0..CHUNK_SIZE {
            for j in 0..CHUNK_SIZE {
                let mut column =
                    culled_mask[axis * CHUNK_SIZE2_U + i as usize * CHUNK_SIZE_U + j as usize];

                while column != 0 {
                    let k = column.trailing_zeros();
                    // clear least significant set bit
                    column &= column - 1;

                    // IGNORE THIS BELOW
                    // get the voxel position based on axis
                    // Basically, you have to look at the x|y|z variable in the part above, and is it in the 1st, 2nd, or 3rd variable spot.
                    // Then you you assign it in this way: 1: i, 2: j, 3: k. Then you put the i|j|k variable in the x|y|z spot below
                    let (x, y, z) = match axis {
                        0 | 1 => (k, i, j), //
                        2 | 3 => (j, k, i), // left, right
                        _ => (i, j, k),     // forward, back
                    };

                    let current_voxel = chunk_refs.get_only_self(x, y, z);
                    dbg!(current_voxel, axis);
                    // let current_voxel = chunks_refs.get_block(voxel_pos);
                    // we can only greedy mesh same block types + same ambient occlusion
                    let block_hash = current_voxel as u32;
                    let layer_data = data[axis] // The 6 axises
                        .entry(block_hash) // This is the type of the block
                        .or_default()
                        .entry(k) // This is what layer of the axis, 0-31
                        .or_default();
                    layer_data[i as usize] |= 1u32 << j; // Setting the entry as 1 at the bit where a face should be
                    // i will be the x in the greedy mesher and j the y
                }
            }
        }
    }

    let mut vertices = Vec::new();
    for (axis_index, axis_data) in data.iter().enumerate() {
        dbg!(axis_data.len());
        let direction = match axis_index {
            0 => FaceDirection::PosX,
            1 => FaceDirection::NegX,
            2 => FaceDirection::PosY,
            3 => FaceDirection::NegY,
            4 => FaceDirection::PosZ,
            _ => FaceDirection::NegZ,
        };
        for (block_hash, block_data) in axis_data.into_iter() {
            let block_type = block_hash;
            for (layer_index, layer) in block_data.into_iter() {
                let quads_from_axis = greedy_mesh(layer);
                for quad in quads_from_axis {
                    let x = quad.x;
                    let y = quad.y;
                    let w = quad.w;
                    let h = quad.h;

                    // 0: Not used
                    // H: Height in greedy meshing
                    // T: Block type
                    // F: Facing direction
                    // Z: Z position in chunk
                    // Y: Y position in chunk
                    // X: X position in chunk
                    // WWWWWHHHHHTTTTFFFZZZZZYYYYYZZZZZ
                    let mut encoded_data: u32 = 0;
                    let pos = match direction {
                        FaceDirection::PosX | FaceDirection::NegX => UVec3::new(*layer_index, x, y), // quad.x -> i -> og y, goes in it's place
                        FaceDirection::PosY | FaceDirection::NegY => UVec3::new(y, *layer_index, x), // quad.x -> i -> og z, goes in it's place
                        FaceDirection::PosZ | FaceDirection::NegZ => UVec3::new(x, y, *layer_index), // quad.x -> i ->  og x, goes in it's place
                    };
                    encoded_data |= pos.x;
                    encoded_data |= pos.y << 5;
                    encoded_data |= pos.z << 10;
                    encoded_data |= (axis_index as u32) << 15;
                    encoded_data |= block_type << 18;
                    encoded_data |= h << 22;
                    encoded_data |= w << 27;
                    vertices.push(Instance(encoded_data));
                }
            }
        }
    }
    vertices
}
