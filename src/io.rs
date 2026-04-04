use crate::chunk::Chunk;
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

    let compressed_data = zstd::encode_all(&chunk.voxels[..], 3)?;

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
    let mut voxels = [0u8; crate::constants::CHUNK_SIZE3_U];

    if decompressed_data.len() != voxels.len() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Decompressed data size mismatch",
        ));
    }

    voxels.copy_from_slice(&decompressed_data);
    Ok(Some(Chunk { voxels }))
}
