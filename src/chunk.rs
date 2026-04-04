use hashbrown::HashMap;

use crate::position::{Ray3, UVec3, Vec3};
use crate::world::World;
use crate::{constants::*, position::IVec3};
use std::sync::Arc;

#[repr(transparent)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, PartialEq, Eq)]
pub struct Instance(pub u32);

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BlockType {
    Empty = 0,
    Dirt = 1,
    Grass = 2,
    Stone = 3,
}

impl From<u8> for BlockType {
    fn from(v: u8) -> Self {
        match v {
            1 => BlockType::Dirt,
            2 => BlockType::Grass,
            3 => BlockType::Stone,
            _ => BlockType::Empty,
        }
    }
}

impl BlockType {
    pub fn is_solid(&self) -> bool {
        *self != BlockType::Empty
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct Quad {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

enum FaceDirection {
    PosX,
    NegX,
    PosY,
    NegY,
    PosZ,
    NegZ,
}

pub fn greedy_mesh(layer: &[u32; 32]) -> Vec<Quad> {
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
pub struct Chunk {
    pub voxels: [u8; CHUNK_SIZE3_U],
}

impl Chunk {
    pub fn new_terrain(position: IVec3, heights: &[i32; CHUNK_SIZE2_U]) -> Self {
        let mut voxels = [BlockType::Empty as u8; CHUNK_SIZE3_U];
        for x in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                let h = heights[x as usize * CHUNK_SIZE_U + z as usize];
                for y in 0..CHUNK_SIZE {
                    let dirt_level = (x as f64).sin() * (z as f64).cos() + 12.0;
                    let global_y = position.y * CHUNK_SIZE as i32 + y as i32;
                    if global_y < h && global_y >= dirt_level as i32 {
                        voxels[UVec3::new(x, y, z).to_index() as usize] = BlockType::Stone as u8;
                    } else if global_y < h {
                        voxels[UVec3::new(x, y, z).to_index() as usize] = BlockType::Dirt as u8;
                    } else if global_y == h {
                        voxels[UVec3::new(x, y, z).to_index() as usize] = BlockType::Grass as u8;
                    }
                }
            }
        }
        Self { voxels }
    }

    pub fn get(&self, position: UVec3) -> BlockType {
        BlockType::from(
            self.voxels[position.x as usize * CHUNK_SIZE2_U
                + position.y as usize * CHUNK_SIZE_U
                + position.z as usize],
        )
    }
}

pub struct ChunkRefs {
    pub refs: [Arc<Chunk>; 27],
}

impl ChunkRefs {
    fn get_from_chunk(&self, position: UVec3, chunk_index: u32) -> BlockType {
        self.refs[chunk_index as usize].get(position)
    }

    pub fn get(&self, position: IVec3) -> BlockType {
        let x = (position.x + 32) as u32;
        let y = (position.y + 32) as u32;
        let z = (position.z + 32) as u32;
        let x_chunk = x >> 5;
        let y_chunk = y >> 5;
        let z_chunk = z >> 5;
        let x = x & 31;
        let y = y & 31;
        let z = z & 31;

        let chunk_index = x_chunk * 9 + y_chunk * 3 + z_chunk;
        self.get_from_chunk(UVec3::new(x, y, z), chunk_index)
    }

    pub fn get_only_self(&self, position: UVec3) -> BlockType {
        self.get_from_chunk(position, 13)
    }
}

pub fn mesh(chunk_refs: ChunkRefs) -> [Vec<Instance>; 6] {
    if chunk_refs.refs[13].voxels == [BlockType::Empty as u8; CHUNK_SIZE3_U] {
        return [const { Vec::new() }; 6];
    }

    let mut occupied_x = [0u64; CHUNK_SIZE2_U];
    let mut occupied_y = [0u64; CHUNK_SIZE2_U];
    let mut occupied_z = [0u64; CHUNK_SIZE2_U];
    let mut culled_mask_x = [0u64; CHUNK_SIZE2_U * 2];
    let mut culled_mask_y = [0u64; CHUNK_SIZE2_U * 2];
    let mut culled_mask_z = [0u64; CHUNK_SIZE2_U * 2];

    for x in -1..CHUNK_SIZE as i32 + 1 {
        for y in -1..CHUNK_SIZE as i32 + 1 {
            for z in -1..CHUNK_SIZE as i32 + 1 {
                if chunk_refs.get(IVec3::new(x, y, z)).is_solid() {
                    let range = 0..32;
                    if range.contains(&y) && range.contains(&z) {
                        occupied_x[(y as usize) * CHUNK_SIZE_U + z as usize] |= 1u64 << x + 1;
                    }
                    if range.contains(&z) && range.contains(&x) {
                        occupied_y[(z as usize) * CHUNK_SIZE_U + x as usize] |= 1u64 << y + 1;
                    }
                    if range.contains(&x) && range.contains(&y) {
                        occupied_z[(x as usize) * CHUNK_SIZE_U + y as usize] |= 1u64 << z + 1;
                    }
                }
            }
        }
    }
    for i in 0..CHUNK_SIZE_U {
        for j in 0..CHUNK_SIZE_U {
            let column_x = occupied_x[i * CHUNK_SIZE_U + j];
            culled_mask_x[i * CHUNK_SIZE_U + j] = column_x & !(column_x >> 1);
            culled_mask_x[CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j] = column_x & !(column_x << 1);
            let column_y = occupied_y[i * CHUNK_SIZE_U + j];
            culled_mask_y[i * CHUNK_SIZE_U + j] = column_y & !(column_y >> 1);
            culled_mask_y[CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j] = column_y & !(column_y << 1);
            let column_z = occupied_z[i * CHUNK_SIZE_U + j];
            culled_mask_z[i * CHUNK_SIZE_U + j] = column_z & !(column_z >> 1);
            culled_mask_z[CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j] = column_z & !(column_z << 1);
        }
    }

    let mut data: [HashMap<u32, HashMap<u32, [u32; 32]>>; 6] = [
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
                let column = match axis {
                    0 | 1 => {
                        culled_mask_x
                            [(axis % 2) * CHUNK_SIZE2_U + i as usize * CHUNK_SIZE_U + j as usize]
                    }
                    2 | 3 => {
                        culled_mask_y
                            [(axis % 2) * CHUNK_SIZE2_U + i as usize * CHUNK_SIZE_U + j as usize]
                    }
                    _ => {
                        culled_mask_z
                            [(axis % 2) * CHUNK_SIZE2_U + i as usize * CHUNK_SIZE_U + j as usize]
                    }
                };
                let mut column = (column >> 1) as u32;

                while column != 0 {
                    let k = column.trailing_zeros();
                    column &= column - 1;

                    let position = match axis {
                        0 | 1 => UVec3::new(k, i, j),
                        2 | 3 => UVec3::new(j, k, i),
                        _ => UVec3::new(i, j, k),
                    };

                    let current_voxel = chunk_refs.get_only_self(position);
                    let texture_id = current_voxel as u32 - 1;

                    let layer_data = data[axis]
                        .entry(texture_id)
                        .or_default()
                        .entry(k)
                        .or_default();
                    layer_data[i as usize] |= 1u32 << j;
                }
            }
        }
    }

    let mut instances = [const { Vec::new() }; 6];
    for (axis_index, axis_data) in data.iter().enumerate() {
        let direction = match axis_index {
            0 => FaceDirection::PosX,
            1 => FaceDirection::NegX,
            2 => FaceDirection::PosY,
            3 => FaceDirection::NegY,
            4 => FaceDirection::PosZ,
            _ => FaceDirection::NegZ,
        };
        for (texture_id, block_data) in axis_data.into_iter() {
            for (layer_index, layer) in block_data.into_iter() {
                let quads_from_axis = greedy_mesh(layer);
                for quad in quads_from_axis {
                    let x = quad.x;
                    let y = quad.y;
                    let w = quad.w;
                    let h = quad.h;

                    let mut encoded_data: u32 = 0;
                    let position = match direction {
                        FaceDirection::PosX | FaceDirection::NegX => UVec3::new(*layer_index, x, y),
                        FaceDirection::PosY | FaceDirection::NegY => UVec3::new(y, *layer_index, x),
                        FaceDirection::PosZ | FaceDirection::NegZ => UVec3::new(x, y, *layer_index),
                    };
                    // Unpack data
                    // WWWWWHHHHHTTTTTTTZZZZZYYYYYXXXXX
                    // X: 0-4 (5 bits)
                    // Y: 5-9 (5 bits)
                    // Z: 10-14 (5 bits)
                    // T: 15-21 (7 bits) - texture data
                    // H: 22-26 (5 bits)
                    // W: 27-31 (5 bits)
                    encoded_data |= position.x;
                    encoded_data |= position.y << 5;
                    encoded_data |= position.z << 10;
                    encoded_data |= (*texture_id) << 15;
                    encoded_data |= (h - 1) << 22; // it won't fit in five bits
                    encoded_data |= (w - 1) << 27; // it won't fit in five bits
                    instances[axis_index].push(Instance(encoded_data));
                }
            }
        }
    }
    instances
}

pub fn raycast(ray: Ray3, world: &World) -> Option<IVec3> {
    let mut position: IVec3 = ray.origin.into_iter().map(|v| v as i32).collect();

    let step: IVec3 = ray
        .reciprical
        .into_iter()
        .map(|v| v.signum() as i32)
        .collect();
    let delta: Vec3 = ray.reciprical.into_iter().map(|v| v.abs()).collect();

    let select: Vec3 = ray
        .reciprical
        .into_iter()
        .map(|v| 0.5 + 0.5 * v.signum())
        .collect();
    let planes: Vec3 = position.into_iter().map(|v| v as f32).collect::<Vec3>() + select;
    let mut t = Vec3::new(
        (planes.x - ray.origin.x) * ray.reciprical.x,
        (planes.y - ray.origin.y) * ray.reciprical.y,
        (planes.z - ray.origin.z) * ray.reciprical.z,
    );

    for _ in 0..1000 {
        let global_position = position.to_chunk_pos();
        if let Some(chunk_refs) = world.get_chunk_refs(global_position) {
            let local_position = position.to_local_pos();
            if chunk_refs.get_only_self(local_position).is_solid() {
                return Some(position);
            }
        }
        if t.x < t.y {
            if t.x < t.z {
                position.x += step.x;
                t.x += delta.x;
            } else {
                position.z += step.z;
                t.z += delta.z;
            }
        } else {
            if t.y < t.z {
                position.y += step.y;
                t.y += delta.y;
            } else {
                position.z += step.z;
                t.z += delta.z;
            }
        }
    }
    None
}
