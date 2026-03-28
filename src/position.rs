use crate::constants::*;
use std::ops::{Add, AddAssign, Div, Mul, MulAssign, Sub, SubAssign};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct IVec3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl IVec3 {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };
    pub const ONE: Self = Self { x: 1, y: 1, z: 1 };

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn splat(v: i32) -> Self {
        Self::new(v, v, v)
    }
}

impl Add for IVec3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}
impl Sub for IVec3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}
impl Mul for IVec3 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
        }
    }
}

impl Div for IVec3 {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
            z: self.z / other.z,
        }
    }
}

impl AddAssign for IVec3 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl SubAssign for IVec3 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl MulAssign for IVec3 {
    fn mul_assign(&mut self, other: Self) {
        *self = *self * other;
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct UVec3 {
    pub x: u32,
    pub y: u32,
    pub z: u32,
}

impl UVec3 {
    pub const ZERO: Self = Self { x: 0, y: 0, z: 0 };
    pub const ONE: Self = Self { x: 1, y: 1, z: 1 };

    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self { x, y, z }
    }

    pub fn splat(v: u32) -> Self {
        Self::new(v, v, v)
    }

    pub fn to_index(&self) -> u32 {
        self.x * CHUNK_SIZE2 + self.y * CHUNK_SIZE + self.z
    }

    pub fn from_index(index: u32) -> Self {
        Self {
            x: index / CHUNK_SIZE2,
            y: (index % CHUNK_SIZE2) / CHUNK_SIZE,
            z: (index % CHUNK_SIZE),
        }
    }
}

impl Add for UVec3 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            x: self.x + other.x,
            y: self.y + other.y,
            z: self.z + other.z,
        }
    }
}

impl Sub for UVec3 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self {
            x: self.x - other.x,
            y: self.y - other.y,
            z: self.z - other.z,
        }
    }
}
impl Mul for UVec3 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        Self {
            x: self.x * other.x,
            y: self.y * other.y,
            z: self.z * other.z,
        }
    }
}

impl Div for UVec3 {
    type Output = Self;

    fn div(self, other: Self) -> Self {
        Self {
            x: self.x / other.x,
            y: self.y / other.y,
            z: self.z / other.z,
        }
    }
}

impl AddAssign for UVec3 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl SubAssign for UVec3 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

impl MulAssign for UVec3 {
    fn mul_assign(&mut self, other: Self) {
        *self = *self * other;
    }
}
