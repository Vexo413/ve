use crate::chunk::Chunk;
use crate::constants::CHUNK_SIZE3_U;
use crate::position::IVec3;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Arc;

pub enum IORequest {
    LoadChunk(IVec3),
    SaveChunk { position: IVec3, chunk: Arc<Chunk> },
}

pub enum IOResponse {
    ChunkLoaded(IVec3, Option<Arc<Chunk>>),
    ChunkSaved(IVec3),
}

pub fn get_chunk_path(pos: IVec3) -> String {
    format!("world/chunks/{}_{}_{}.bin", pos.x, pos.y, pos.z)
}

pub fn save_chunk(position: IVec3, chunk: &Chunk) -> std::io::Result<()> {
    let path_str = get_chunk_path(position);
    let path = Path::new(&path_str);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let voxels_u8: Vec<u8> = chunk.voxels.iter().map(|&v| v as u8).collect();
    let compressed_data = zstd::encode_all(&voxels_u8[..], 3)?;

    let mut file = fs::File::create(path)?;
    file.write_all(&compressed_data)?;
    Ok(())
}

pub fn load_chunk(pos: IVec3) -> std::io::Result<Option<Chunk>> {
    let path_str = get_chunk_path(pos);
    let path = Path::new(&path_str);
    if !path.exists() {
        return Ok(None);
    }

    let mut file = fs::File::open(path)?;
    let mut compressed_data = Vec::new();
    file.read_to_end(&mut compressed_data)?;

    let decompressed_data = zstd::decode_all(&compressed_data[..])?;
    let mut voxels = [0u32; CHUNK_SIZE3_U];

    if decompressed_data.len() == voxels.len() * 4 {
        voxels.copy_from_slice(bytemuck::cast_slice(&decompressed_data));
    } else {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Decompressed data size mismatch: expected {} or {}, got {}",
                voxels.len(),
                voxels.len() * 4,
                decompressed_data.len()
            ),
        ));
    }

    Ok(Some(Chunk { voxels }))
}
