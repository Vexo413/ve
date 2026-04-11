use crate::constants::*;
pub use glam::{IVec3, UVec3, Vec3};

pub trait IVec3Ext {
    fn to_chunk_pos(&self) -> IVec3;
    fn to_local_pos(&self) -> UVec3;
}

impl IVec3Ext for IVec3 {
    fn to_chunk_pos(&self) -> IVec3 {
        IVec3::new(
            self.x.div_euclid(CHUNK_SIZE as i32),
            self.y.div_euclid(CHUNK_SIZE as i32),
            self.z.div_euclid(CHUNK_SIZE as i32),
        )
    }

    fn to_local_pos(&self) -> UVec3 {
        UVec3::new(
            self.x.rem_euclid(CHUNK_SIZE as i32) as u32,
            self.y.rem_euclid(CHUNK_SIZE as i32) as u32,
            self.z.rem_euclid(CHUNK_SIZE as i32) as u32,
        )
    }
}

pub trait UVec3Ext {
    fn to_index(&self) -> u32;
    fn from_index(index: u32) -> Self;
}

impl UVec3Ext for UVec3 {
    fn to_index(&self) -> u32 {
        self.x * CHUNK_SIZE2 + self.y * CHUNK_SIZE + self.z
    }

    fn from_index(index: u32) -> Self {
        Self {
            x: index / CHUNK_SIZE2,
            y: (index % CHUNK_SIZE2) / CHUNK_SIZE,
            z: (index % CHUNK_SIZE),
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Ray3 {
    pub origin: Vec3,
    pub direction: Vec3,
    pub reciprocal: Vec3,
}

impl Ray3 {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction,
            reciprocal: direction.recip(),
        }
    }

    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}
