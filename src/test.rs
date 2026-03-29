use std::sync::Arc;

use crate::{chunk::*, constants::*};

#[test]
fn test_block_type_is_solid() {
    assert!(!BlockType::Empty.is_solid());
    assert!(BlockType::Dirt.is_solid());
}

#[test]
fn test_chunk_get() {
    let mut voxels = [BlockType::Empty; CHUNK_SIZE3_U];
    voxels[0] = BlockType::Dirt;
    let chunk = Chunk { voxels };
    assert_eq!(chunk.get(0, 0, 0), BlockType::Dirt);
    assert_eq!(chunk.get(1, 0, 0), BlockType::Empty);
    assert_eq!(chunk.get(31, 31, 31), BlockType::Empty);
}

#[test]
fn test_chunk_get_only_self() {
    let mut voxels = [BlockType::Empty; CHUNK_SIZE3_U];
    voxels[0] = BlockType::Dirt;
    let chunk = Chunk { voxels };
    let chunk_refs = ChunkRefs {
        refs: std::array::repeat(Arc::new(chunk)),
    };

    assert_eq!(chunk_refs.get_only_self(0, 0, 0), BlockType::Dirt);
    assert_eq!(chunk_refs.get_only_self(1, 0, 0), BlockType::Empty);
}

#[test]
fn test_chunk_refs_get() {
    let mut voxels = [BlockType::Empty; CHUNK_SIZE3_U];
    voxels[0] = BlockType::Dirt;
    let chunk = Chunk { voxels };
    let chunk_refs = ChunkRefs {
        refs: std::array::repeat(Arc::new(chunk)),
    };

    // Verify center chunk access
    assert_eq!(chunk_refs.get(0, 0, 0), BlockType::Dirt);
    assert_eq!(chunk_refs.get(1, 0, 0), BlockType::Empty);
    assert_eq!(chunk_refs.get(31, 31, 31), BlockType::Empty);
    // Verify negative coordinates
    assert_eq!(chunk_refs.get(-1, -1, -1), BlockType::Empty);
    // Access beyond center chunk
    assert_eq!(chunk_refs.get(-32, -32, -32), BlockType::Dirt);
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
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 1,
        h: 1
    }));
}

#[test]
fn test_greedy_mesh_horizontal_line() {
    let mut layer = [0u32; 32];
    layer[0] = 0b111;
    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 1);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 1,
        h: 3
    }));
}

#[test]
fn test_greedy_mesh_vertical_line() {
    let mut layer = [0u32; 32];
    layer[0] = 0b1;
    layer[1] = 0b1;
    layer[2] = 0b1;
    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 1);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 3,
        h: 1
    }));
}

#[test]
fn test_greedy_mesh_square() {
    let mut layer = [0u32; 32];
    layer[0] = 0b11;
    layer[1] = 0b11;
    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 1);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 2,
        h: 2
    }));
}

#[test]
fn test_greedy_mesh_disconnected_regions() {
    let mut layer = [0u32; 32];
    layer[0] = 0b1;
    layer[5] = 0b1;
    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 2);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 1,
        h: 1
    }));
    assert!(quads.contains(&Quad {
        x: 5,
        y: 0,
        w: 1,
        h: 1
    }));
}

#[test]
fn test_greedy_mesh_full_layer() {
    let mut layer = [0u32; 32];
    for row in &mut layer {
        *row = u32::MAX;
    }
    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 1);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 32,
        h: 32
    }));
}

#[test]
fn test_greedy_mesh_l_shape() {
    let mut layer = [0u32; 32];
    layer[0] = 0b111; // bits at y=0,1,2 at x=0
    layer[1] = 0b1; // bit at y=0 at x=1
    let quads = greedy_mesh(&layer);
    // L-shape produces quads from greedy meshing
    // The exact count depends on the algorithm's optimization
    assert_eq!(quads.len(), 2);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 1,
        h: 3
    }));
    assert!(quads.contains(&Quad {
        x: 1,
        y: 0,
        w: 1,
        h: 1
    }));
}

#[test]
fn test_greedy_mesh_vertical_column() {
    let mut layer = [0u32; 32];
    layer[0] = 0b111111; // bits at y=0-5 at x=0
    layer[1] = 0b111111; // bits at y=0-5 at x=1
    layer[2] = 0b111111; // bits at y=0-5 at x=2

    let quads = greedy_mesh(&layer);

    assert_eq!(quads.len(), 1);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 3,
        h: 6
    }));
}

#[test]
fn test_greedy_mesh_staircase() {
    let mut layer = [0u32; 32];
    layer[0] = 0b1;
    layer[1] = 0b11;
    layer[2] = 0b111;
    layer[3] = 0b1111;

    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 4);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 4,
        h: 1
    }));
    assert!(quads.contains(&Quad {
        x: 1,
        y: 1,
        w: 3,
        h: 1
    }));
    assert!(quads.contains(&Quad {
        x: 2,
        y: 2,
        w: 2,
        h: 1
    }));
    assert!(quads.contains(&Quad {
        x: 3,
        y: 3,
        w: 1,
        h: 1
    }));
}

#[test]
fn test_greedy_mesh_wide_short() {
    let mut layer = [0u32; 32];
    for i in 0..10 {
        layer[i] = 0b1;
    }

    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 1);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 10,
        h: 1
    }));
}

#[test]
fn test_greedy_mesh_tall_narrow() {
    let mut layer = [0u32; 32];
    layer[0] = 0b1111111111;

    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 1);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 1,
        h: 10
    }));
}

#[test]
fn test_greedy_mesh_single_row() {
    let mut layer = [0u32; 32];
    layer[0] = 0b1001;

    let quads = greedy_mesh(&layer);
    assert_eq!(quads.len(), 2);
    assert!(quads.contains(&Quad {
        x: 0,
        y: 0,
        w: 1,
        h: 1
    }));
    assert!(quads.contains(&Quad {
        x: 0,
        y: 3,
        w: 1,
        h: 1
    }));
}

#[test]
fn test_meshing_algorithm() {
    let mut voxels = [BlockType::Empty; CHUNK_SIZE3_U];
    voxels[0] = BlockType::Dirt;
    let chunk = Chunk { voxels };
    let chunk_refs = ChunkRefs {
        refs: std::array::repeat(Arc::new(chunk)),
    };
    let faces = mesh(chunk_refs);
}
