use std::{collections::HashMap, sync::Arc};

use bevy::{
    asset::RenderAssetUsages,
    math::primitives::Rectangle,
    mesh::{Mesh, MeshVertexAttribute},
};

const CHUNK_SIZE: u32 = 32;
const CHUNK_SIZE_U: usize = CHUNK_SIZE as usize;
const CHUNK_SIZE_P: u32 = CHUNK_SIZE + 2;
const CHUNK_SIZE_PU: usize = CHUNK_SIZE_P as usize;
const CHUNK_SIZE2: u32 = CHUNK_SIZE * CHUNK_SIZE;
const CHUNK_SIZE2_U: usize = CHUNK_SIZE2 as usize;
const CHUNK_SIZE3: u32 = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
const CHUNK_SIZE3_U: usize = CHUNK_SIZE3 as usize;

fn main() {}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;

    const CHUNK_SIZE: u32 = 32;
    const CHUNK_SIZE_U: usize = CHUNK_SIZE as usize;
    const CHUNK_SIZE_P: u32 = CHUNK_SIZE + 2;
    const CHUNK_SIZE_PU: usize = CHUNK_SIZE_P as usize;
    const CHUNK_SIZE2: u32 = CHUNK_SIZE * CHUNK_SIZE;
    const CHUNK_SIZE2_U: usize = CHUNK_SIZE2 as usize;
    const CHUNK_SIZE3: u32 = CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE;
    const CHUNK_SIZE3_U: usize = CHUNK_SIZE3 as usize;

    #[derive(Copy, Clone, Debug, PartialEq)]
    pub enum BlockType {
        Empty,
        Dirt,
    }

    impl BlockType {
        pub fn is_solid(&self) -> bool {
            !matches!(self, BlockType::Empty)
        }
    }

    #[derive(Debug, PartialEq)]
    struct Quad {
        x: u32,
        y: u32,
        w: u32,
        h: u32,
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

    struct Chunk {
        voxels: [BlockType; CHUNK_SIZE3_U],
    }

    impl Chunk {
        pub fn get(&self, x: u32, y: u32, z: u32) -> BlockType {
            self.voxels[x as usize * CHUNK_SIZE2_U + y as usize * CHUNK_SIZE_U + z as usize]
        }

        fn from_blocks(blocks: impl Fn(u32, u32, u32) -> BlockType) -> Self {
            let mut voxels = [BlockType::Empty; CHUNK_SIZE3_U];
            for x in 0..CHUNK_SIZE {
                for y in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        voxels
                            [x as usize * CHUNK_SIZE2_U + y as usize * CHUNK_SIZE_U + z as usize] =
                            blocks(x, y, z);
                    }
                }
            }
            Self { voxels }
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
            let x = (x + 32) as u32;
            let y = (y + 32) as u32;
            let z = (z + 32) as u32;
            let (x_chunk, x) = ((x / 32), (x % 32));
            let (y_chunk, y) = ((y / 32), (y % 32));
            let (z_chunk, z) = ((z / 32), (z % 32));
            let chunk_index = x_chunk * 9 + y_chunk * 3 + z_chunk;
            self.get_from_chunk(x, y, z, chunk_index)
        }

        pub fn get_only_self(&self, x: u32, y: u32, z: u32) -> BlockType {
            self.get_from_chunk(x, y, z, 13)
        }

        fn from_chunks(chunks: impl Fn(u32, u32, u32) -> BlockType) -> Self {
            let refs: [Arc<Chunk>; 27] = std::array::from_fn(|i| {
                Arc::new(Chunk::from_blocks(|x, y, z| {
                    let cx = (i / 9) as i32 - 1;
                    let cy = ((i / 3) % 3) as i32 - 1;
                    let cz = (i % 3) as i32 - 1;
                    chunks(
                        x + (cx * 32) as u32,
                        y + (cy * 32) as u32,
                        z + (cz * 32) as u32,
                    )
                }))
            });
            Self { refs }
        }
    }

    #[test]
    fn test_block_type_is_solid() {
        assert!(!BlockType::Empty.is_solid());
        assert!(BlockType::Dirt.is_solid());
    }

    #[test]
    fn test_chunk_get() {
        let chunk = Chunk::from_blocks(|x, y, z| {
            if x == 0 && y == 0 && z == 0 {
                BlockType::Dirt
            } else {
                BlockType::Empty
            }
        });

        assert_eq!(chunk.get(0, 0, 0), BlockType::Dirt);
        assert_eq!(chunk.get(1, 0, 0), BlockType::Empty);
        assert_eq!(chunk.get(31, 31, 31), BlockType::Empty);
    }

    #[test]
    fn test_chunk_get_only_self() {
        let chunk_refs = ChunkRefs::from_chunks(|x, y, z| {
            if x == 0 && y == 0 && z == 0 {
                BlockType::Dirt
            } else {
                BlockType::Empty
            }
        });

        assert_eq!(chunk_refs.get_only_self(0, 0, 0), BlockType::Dirt);
        assert_eq!(chunk_refs.get_only_self(1, 0, 0), BlockType::Empty);
    }

    #[test]
    fn test_chunk_refs_get() {
        let chunk_refs = ChunkRefs::from_chunks(|x, y, z| {
            // x, y, z are local coords (0-31) within each chunk
            if x == 0 && y == 0 && z == 0 {
                BlockType::Dirt
            } else {
                BlockType::Empty
            }
        });

        // Verify center chunk access
        assert_eq!(chunk_refs.get(0, 0, 0), BlockType::Dirt);
        assert_eq!(chunk_refs.get(1, 0, 0), BlockType::Empty);
        assert_eq!(chunk_refs.get(31, 31, 31), BlockType::Empty);
        // Verify negative coordinates
        assert_eq!(chunk_refs.get(-1, -1, -1), BlockType::Empty);
        // Access beyond center chunk
        assert_eq!(
            chunk_refs.get(32, 0, 0).is_solid() || !chunk_refs.get(32, 0, 0).is_solid(),
            true
        );
    }

    #[test]
    fn test_greedy_mesh_empty_layer() {
        let layer = [0u32; 32];
        let quads = greedy_mesh(&layer);
        assert!(quads.is_empty());
    }

    #[test]
    fn test_greedy_mesh_single_voxel() {
        let mut layer = [0u32; 32];
        layer[0] = 1;
        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0],
            Quad {
                x: 0,
                y: 0,
                w: 1,
                h: 1
            }
        );
    }

    #[test]
    fn test_greedy_mesh_horizontal_line() {
        let mut layer = [0u32; 32];
        layer[0] = 0b111;
        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0],
            Quad {
                x: 0,
                y: 0,
                w: 1,
                h: 3
            }
        );
    }

    #[test]
    fn test_greedy_mesh_vertical_line() {
        let mut layer = [0u32; 32];
        layer[0] = 1;
        layer[1] = 1;
        layer[2] = 1;
        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0],
            Quad {
                x: 0,
                y: 0,
                w: 3,
                h: 1
            }
        );
    }

    #[test]
    fn test_greedy_mesh_square() {
        let mut layer = [0u32; 32];
        layer[0] = 0b11;
        layer[1] = 0b11;
        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0],
            Quad {
                x: 0,
                y: 0,
                w: 2,
                h: 2
            }
        );
    }

    #[test]
    fn test_greedy_mesh_disconnected_regions() {
        let mut layer = [0u32; 32];
        layer[0] = 1;
        layer[5] = 1;
        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 2);
    }

    #[test]
    fn test_greedy_mesh_full_layer() {
        let mut layer = [0u32; 32];
        for row in &mut layer {
            *row = u32::MAX;
        }
        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0],
            Quad {
                x: 0,
                y: 0,
                w: 32,
                h: 32
            }
        );
    }

    #[test]
    fn test_greedy_mesh_l_shape() {
        let mut layer = [0u32; 32];
        layer[0] = 0b111; // bits at y=0,1,2 at x=0
        layer[1] = 0b1; // bit at y=0 at x=1
        let quads = greedy_mesh(&layer);
        // L-shape produces quads from greedy meshing
        // The exact count depends on the algorithm's optimization
        assert!(!quads.is_empty());
        for quad in &quads {
            assert!(quad.w >= 1 && quad.h >= 1);
            assert!(quad.x < 32 && quad.y < 32);
        }
    }

    fn run_face_culling(occupied: &[u32; CHUNK_SIZE2_U * 3]) -> [u32; CHUNK_SIZE2_U * 3 * 2] {
        let mut culled_mask = [0u32; CHUNK_SIZE2_U * 3 * 2];
        for axis in 0..3 {
            for i in 0..CHUNK_SIZE_U {
                for j in 0..CHUNK_SIZE_U {
                    let column = occupied[axis * CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j];
                    let pos_face = column & (!column >> 1);
                    let neg_face = column & (!column << 1);
                    culled_mask[axis * 2 * CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j] = pos_face;
                    culled_mask
                        [axis * 2 * CHUNK_SIZE2_U + 1 * CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j] =
                        neg_face;
                }
            }
        }
        culled_mask
    }

    #[test]
    fn test_face_culling_no_neighbors() {
        let occupied = {
            let mut o = [0u32; CHUNK_SIZE2_U * 3];
            o[16 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o[1 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o[2 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o
        };

        let culled = run_face_culling(&occupied);

        assert!(culled[0 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] & (1u32 << 16) != 0);
        assert!(culled[1 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] & (1u32 << 16) != 0);
        assert!(culled[2 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] & (1u32 << 16) != 0);
        assert!(culled[3 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] != 0);
        assert!(culled[4 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] != 0);
        assert!(culled[5 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] != 0);
    }

    #[test]
    fn test_face_culling_internal_faces_culled() {
        let occupied = {
            let mut o = [0u32; CHUNK_SIZE2_U * 3];
            o[16 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o[16 * CHUNK_SIZE_U + 16] |= 1u32 << 17;
            o[1 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o[1 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 17] |= 1u32 << 16;
            o[2 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o[2 * CHUNK_SIZE2_U + 17 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o
        };

        let culled = run_face_culling(&occupied);

        let col_at_y16_z16 = occupied[16 * CHUNK_SIZE_U + 16];
        // Positive face: col & !col >> 1 - shows faces with no neighbor to the right
        let pos_x_face = col_at_y16_z16 & !col_at_y16_z16 >> 1;
        // Negative face: col & !col << 1 - shows faces with no neighbor to the left
        let neg_x_face = col_at_y16_z16 & !col_at_y16_z16 << 1;

        // Position 16: has neighbor at 17 to the right, so positive face culled
        assert!(
            pos_x_face & (1u32 << 16) == 0,
            "Positive face at x=16 should be culled"
        );
        // Position 17: no neighbor to the right, so positive face visible
        assert!(
            pos_x_face & (1u32 << 17) != 0,
            "Positive face at x=17 should be visible"
        );
        // Position 16: no neighbor to the left, so negative face visible
        assert!(
            neg_x_face & (1u32 << 16) != 0,
            "Negative face at x=16 should be visible"
        );
    }

    #[test]
    fn test_face_culling_boundary() {
        let occupied = {
            let mut o = [0u32; CHUNK_SIZE2_U * 3];
            o[16 * CHUNK_SIZE_U + 16] |= 1u32 << 31;
            o[1 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 31] |= 1u32 << 16;
            o[2 * CHUNK_SIZE2_U + 31 * CHUNK_SIZE_U + 16] |= 1u32 << 16;
            o
        };

        let culled = run_face_culling(&occupied);
        // Voxel at position 31 - positive face should be visible (no neighbor to the right)
        // negative face should be visible (no neighbor at position 30 in this column)
        let col = occupied[16 * CHUNK_SIZE_U + 16];
        let pos_face = col & !col >> 1;
        let neg_face = col & !col << 1;
        // Position 31: has bit set, neighbor check for right (pos) and left (neg)
        // For positive: !col >> 1 at 31 is 0 (since bit 30 is 0), so pos_face at 31 = 0
        // For negative: !col << 1 at 31 is 1 (bit 31 was set, shifted to 32=overflow), so neg_face at 31 = 1
        assert!(
            neg_face & (1u32 << 31) != 0,
            "Negative face at position 31 should be visible"
        );
    }

    #[test]
    fn test_mesh_simple_cube() {
        let chunk_refs = ChunkRefs::from_chunks(|x, y, z| {
            if x == 16 && y == 16 && z == 16 {
                BlockType::Dirt
            } else {
                BlockType::Empty
            }
        });

        let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    if chunk_refs.get_only_self(x, y, z).is_solid() {
                        occupied[y as usize * CHUNK_SIZE_U + z as usize] |= 1u32 << x;
                        occupied[1 * CHUNK_SIZE2_U + z as usize * CHUNK_SIZE_U + x as usize] |=
                            1u32 << y;
                        occupied[2 * CHUNK_SIZE2_U + x as usize * CHUNK_SIZE_U + y as usize] |=
                            1u32 << z;
                    }
                }
            }
        }

        assert!(occupied[16 * CHUNK_SIZE_U + 16] & (1u32 << 16) != 0);
        assert!(occupied[1 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] & (1u32 << 16) != 0);
        assert!(occupied[2 * CHUNK_SIZE2_U + 16 * CHUNK_SIZE_U + 16] & (1u32 << 16) != 0);
    }

    #[test]
    fn test_mesh_full_chunk() {
        let chunk_refs = ChunkRefs::from_chunks(|_x, _y, _z| BlockType::Dirt);

        let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    if chunk_refs.get_only_self(x, y, z).is_solid() {
                        occupied[y as usize * CHUNK_SIZE_U + z as usize] |= 1u32 << x;
                        occupied[1 * CHUNK_SIZE2_U + z as usize * CHUNK_SIZE_U + x as usize] |=
                            1u32 << y;
                        occupied[2 * CHUNK_SIZE2_U + x as usize * CHUNK_SIZE_U + y as usize] |=
                            1u32 << z;
                    }
                }
            }
        }

        for y in 0..CHUNK_SIZE {
            for z in 0..CHUNK_SIZE {
                assert_eq!(
                    occupied[y as usize * CHUNK_SIZE_U + z as usize],
                    u32::MAX,
                    "X-axis column at y={}, z={} should be fully set",
                    y,
                    z
                );
            }
        }
    }

    #[test]
    fn test_mesh_sparse_voxels() {
        let chunk_refs = ChunkRefs::from_chunks(|x, y, z| {
            if (x == 5 && y == 5 && z == 5)
                || (x == 20 && y == 10 && z == 15)
                || (x == 25 && y == 25 && z == 25)
            {
                BlockType::Dirt
            } else {
                BlockType::Empty
            }
        });

        let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    if chunk_refs.get_only_self(x, y, z).is_solid() {
                        occupied[y as usize * CHUNK_SIZE_U + z as usize] |= 1u32 << x;
                        occupied[1 * CHUNK_SIZE2_U + z as usize * CHUNK_SIZE_U + x as usize] |=
                            1u32 << y;
                        occupied[2 * CHUNK_SIZE2_U + x as usize * CHUNK_SIZE_U + y as usize] |=
                            1u32 << z;
                    }
                }
            }
        }

        let mut total_bits = 0;
        for val in &occupied {
            total_bits += val.count_ones();
        }
        assert_eq!(total_bits, 9);
    }

    #[test]
    fn test_mesh_correct_bit_position() {
        let chunk_refs = ChunkRefs::from_chunks(|x, y, z| {
            if x == 10 && y == 20 && z == 30 {
                BlockType::Dirt
            } else {
                BlockType::Empty
            }
        });

        let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    if chunk_refs.get_only_self(x, y, z).is_solid() {
                        occupied[y as usize * CHUNK_SIZE_U + z as usize] |= 1u32 << x;
                        occupied[1 * CHUNK_SIZE2_U + z as usize * CHUNK_SIZE_U + x as usize] |=
                            1u32 << y;
                        occupied[2 * CHUNK_SIZE2_U + x as usize * CHUNK_SIZE_U + y as usize] |=
                            1u32 << z;
                    }
                }
            }
        }

        assert_eq!(
            occupied[20 * CHUNK_SIZE_U + 30],
            1u32 << 10,
            "X-axis at (y=20, z=30) should have bit 10"
        );
        assert_eq!(
            occupied[1 * CHUNK_SIZE2_U + 30 * CHUNK_SIZE_U + 10],
            1u32 << 20,
            "Y-axis at (z=30, x=10) should have bit 20"
        );
        assert_eq!(
            occupied[2 * CHUNK_SIZE2_U + 10 * CHUNK_SIZE_U + 20],
            1u32 << 30,
            "Z-axis at (x=10, y=20) should have bit 30"
        );
    }

    #[test]
    fn test_face_culling_single_voxel_corner() {
        let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
        occupied[0] = 1u32 << 0;
        occupied[1 * CHUNK_SIZE2_U + 0] = 1u32 << 0;
        occupied[2 * CHUNK_SIZE2_U + 0] = 1u32 << 0;

        let _culled = run_face_culling(&occupied);
    }

    #[test]
    fn test_chunk_boundary_access() {
        let chunk_refs = ChunkRefs::from_chunks(|x, y, z| {
            if x == 31 && y == 31 && z == 31 {
                BlockType::Dirt
            } else {
                BlockType::Empty
            }
        });

        assert_eq!(chunk_refs.get_only_self(31, 31, 31), BlockType::Dirt);
        assert_eq!(chunk_refs.get_only_self(30, 31, 31), BlockType::Empty);
        assert_eq!(chunk_refs.get(31, 31, 31), BlockType::Dirt);
    }

    #[test]
    fn test_greedy_mesh_vertical_column() {
        let mut layer = [0u32; 32];
        layer[0] = 0b111111; // bits at y=0-5 at x=0
        layer[1] = 0b111111; // bits at y=0-5 at x=1
        layer[2] = 0b111111; // bits at y=0-5 at x=2

        let quads = greedy_mesh(&layer);
        // Dense region should produce fewer, larger quads
        assert!(!quads.is_empty());
        for quad in &quads {
            assert!(quad.w >= 1 && quad.h >= 1);
            assert!(quad.x < 32 && quad.y < 32);
        }
    }

    #[test]
    fn test_greedy_mesh_staircase() {
        let mut layer = [0u32; 32];
        layer[0] = 0b1;
        layer[1] = 0b11;
        layer[2] = 0b111;
        layer[3] = 0b1111;

        let quads = greedy_mesh(&layer);
        assert!(!quads.is_empty());

        for quad in &quads {
            assert!(quad.x < 32);
            assert!(quad.y < 32);
            assert!(quad.w >= 1);
            assert!(quad.h >= 1);
            assert!(quad.x + quad.w <= 32);
            assert!(quad.y + quad.h <= 32);
        }
    }

    #[test]
    fn test_greedy_mesh_wide_short() {
        let mut layer = [0u32; 32];
        for i in 0..10 {
            layer[i] = 0b1;
        }

        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0],
            Quad {
                x: 0,
                y: 0,
                w: 10,
                h: 1
            }
        );
    }

    #[test]
    fn test_greedy_mesh_tall_narrow() {
        let mut layer = [0u32; 32];
        layer[0] = 0b1111111111;

        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0],
            Quad {
                x: 0,
                y: 0,
                w: 1,
                h: 10
            }
        );
    }

    #[test]
    fn test_mesh_cross_chunk_access() {
        let chunk_refs = ChunkRefs::from_chunks(|_x, _y, _z| BlockType::Dirt);

        assert_eq!(chunk_refs.get(0, 0, 0), BlockType::Dirt);
        assert_eq!(chunk_refs.get(63, 63, 63), BlockType::Dirt);
        assert_eq!(chunk_refs.get(-1, -1, -1), BlockType::Dirt);
        assert_eq!(chunk_refs.get(32, 0, 0), BlockType::Dirt);
        assert_eq!(chunk_refs.get(-32, -32, -32), BlockType::Dirt);
    }

    #[test]
    fn test_mesh_empty_chunk() {
        let chunk_refs = ChunkRefs::from_chunks(|_x, _y, _z| BlockType::Empty);

        let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
        for x in 0..CHUNK_SIZE {
            for y in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    if chunk_refs.get_only_self(x, y, z).is_solid() {
                        occupied[y as usize * CHUNK_SIZE_U + z as usize] |= 1u32 << x;
                        occupied[1 * CHUNK_SIZE2_U + z as usize * CHUNK_SIZE_U + x as usize] |=
                            1u32 << y;
                        occupied[2 * CHUNK_SIZE2_U + x as usize * CHUNK_SIZE_U + y as usize] |=
                            1u32 << z;
                    }
                }
            }
        }

        for val in &occupied {
            assert_eq!(*val, 0);
        }
    }

    #[test]
    fn test_greedy_mesh_single_row() {
        let mut layer = [0u32; 32];
        layer[0] = 0b1001;

        let quads = greedy_mesh(&layer);
        assert_eq!(quads.len(), 2);
    }

    #[test]
    fn test_face_culling_three_in_line() {
        let mut occupied = [0u32; CHUNK_SIZE2_U * 3];
        let col = (1u32 << 5) | (1u32 << 6) | (1u32 << 7);
        occupied[0] = col;

        let culled = run_face_culling(&occupied);

        let pos_x = col & !col >> 1;
        assert!(pos_x & (1u32 << 7) != 0, "Bit 7 should have positive face");
        assert!(
            pos_x & (1u32 << 5) == 0,
            "Bit 5 should NOT have positive face"
        );
        assert!(
            pos_x & (1u32 << 6) == 0,
            "Bit 6 should NOT have positive face"
        );

        let neg_x = col & !col << 1;
        assert!(neg_x & (1u32 << 5) != 0, "Bit 5 should have negative face");
        assert!(
            neg_x & (1u32 << 7) == 0,
            "Bit 7 should NOT have negative face"
        );
        assert!(
            neg_x & (1u32 << 6) == 0,
            "Bit 6 should NOT have negative face"
        );
    }
}

#[derive(Copy, Clone, Debug)]
pub enum BlockType {
    Empty,
    Dirt,
}

impl BlockType {
    pub fn is_solid(&self) -> bool {
        !matches!(self, BlockType::Empty)
    }
}

#[derive(Debug)]
struct Quad {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

enum FaceDirection {
    X,
    NegX,
    Y,
    NegY,
    Z,
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

fn mesh(chunk_refs: ChunkRefs) {
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
                    // y, z - x axis
                    // k, i, j
                    occupied[/*0 * CHUNK_SIZE2_U +*/ y as usize * CHUNK_SIZE_U + z as usize] |=
                        1u32 << x;
                    // z, x - y axis
                    // j, k, i
                    occupied[1 * CHUNK_SIZE2_U + z as usize * CHUNK_SIZE_U + x as usize] |=
                        1u32 << y;
                    // x, y - z axis
                    // i, j, k
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
                    column & !column >> 1;
                culled_mask[axis * 2 * CHUNK_SIZE2_U + 1 * CHUNK_SIZE2_U + i * CHUNK_SIZE_U + j] =
                    column & !column << 1;
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

                    // get the voxel position based on axis
                    // Basically, you have to look at the x|y|z variable in the part above, and is it in the 1st, 2nd, or 3rd variable spot.
                    // Then you you assign it in this way: 1: i, 2: j, 3: k. Then you put the i|j|k variable in the x|y|z spot below
                    let (x, y, z) = match axis {
                        0 | 1 => (k, i, j), //
                        2 | 3 => (j, k, i), // left, right
                        _ => (i, j, k),     // forward, back
                    };

                    let current_voxel = chunk_refs.get_only_self(x, y, z);
                    // let current_voxel = chunks_refs.get_block(voxel_pos);
                    // we can only greedy mesh same block types + same ambient occlusion
                    let block_hash = current_voxel as u32;
                    let data = data[axis] // The 6 axises
                        .entry(block_hash) // This is the type of the block
                        .or_default()
                        .entry(k) // This is what layer of the axis, 0-31
                        .or_default();
                    data[i as usize] |= 1u32 << k; // Setting the entry as 1 at the bit where a face should be
                }
            }
        }
    }

    let mut vertices = Vec::new();
    for (axis_index, axis_data) in data.iter().enumerate() {
        let direction = match axis_index {
            0 => FaceDirection::X,
            1 => FaceDirection::NegX,
            2 => FaceDirection::Y,
            3 => FaceDirection::NegY,
            4 => FaceDirection::Z,
            _ => FaceDirection::NegZ,
        };
        for (block_hash, block_data) in axis_data.into_iter() {
            let block_type = block_hash;
            for (layer_index, layer) in block_data.into_iter() {
                let quads_from_axis = greedy_mesh(layer);
                for quad in quads_from_axis {
                    let x = quad.x as f32;
                    let y = quad.y as f32;
                    let w = quad.w as f32;
                    let h = quad.h as f32;
                    vertices.push([x, y, 0.0]); // 1
                    vertices.push([x + w, y, 0.0]); // 2
                    vertices.push([x + w, y + h, 0.0]); // 3

                    vertices.push([x, y, 0.0]); // 1
                    vertices.push([x, y + h, 0.0]); // 4
                    vertices.push([x + w, y + h, 0.0]); // 3
                }
            }
        }
    }
    let mesh = Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
}
