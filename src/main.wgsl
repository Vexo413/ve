struct InstanceInput {
    @location(0) data1: u32,
    @location(1) data2: u32,
    @location(2) chunk_id: u32,
};

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) face: u32,
    @location(2) @interpolate(flat) texture_id: u32,
    @location(3) ao: f32,
};

struct ChunkData {
    world_pos: vec3<f32>,
    base_instance_id: u32,
    face_counts: array<u32, 6>,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<storage, read> chunks_data: array<ChunkData>;
@group(1) @binding(0)
var<uniform> camera: CameraUniform;
@group(2) @binding(0)
var t_diffuse: texture_2d<f32>;
@group(2) @binding(1)
var s_diffuse: sampler;

@vertex
fn vs_main(
    instance: InstanceInput,
    vertex: VertexInput,
) -> VertexOutput {
    let data1 = instance.data1;
    let data2 = instance.data2;
    let chunk_id = instance.chunk_id;
    let chunk = chunks_data[chunk_id];
    
    // Pack data
    // TTTTTTTTTTTTTTTTTTTTTTTTTTTTTTTAAAAAAAAHHHHHWWWWWZZZZZYYYYYXXXXX
    // X: 0..5 (5 bits)
    // Y: 5..10 (5 bits)
    // Z: 10..15 (5 bits)
    // W: 15..20 (5 bits)
    // H: 20..25 (5 bits)
    // A: 25..33 (8 bits) - total 33 bits
    // T: 33..64 (31 bits)
    let x = f32(data1 & 0x1Fu);
    let y = f32((data1 >> 5u) & 0x1Fu);
    let z = f32((data1 >> 10u) & 0x1Fu);
    let w = f32((data1 >> 15u) & 0x1Fu) + 1.0;
    let h = f32((data1 >> 20u) & 0x1Fu) + 1.0;
    let ao = (data1 >> 25u) & 0x7Fu | (data2 & 0x1u) << 7; // One less bit, then one bit
    let t = (data2 >> 1u) & 0x7FFFFFFFu;

    // Calculate face from ChunkData and instance_index
    let local_id = vertex.instance_index - chunk.base_instance_id;
    var face = 0u;
    var count_acc = 0u;
    for (var i = 0u; i < 6u; i++) {
        count_acc                                                                                                                 += chunk.face_counts[i];
        if local_id < count_acc {
            face = i;
            break;
        }
    }

    var pos: vec3<f32>;
    let quad_pos = vec2<f32>(
        f32(vertex.vertex_index % 2u),
        f32(vertex.vertex_index / 2u)
    );
    
    // quad_pos is (0,0), (1,0), (0,1), (1,1)
    
    // For negative faces, we need to swap w and h and change the vertex order to keep CCW winding
    var w_eff = w;
    var h_eff = h;
    if face % 2u == 1u {
        w_eff = h;
        h_eff = w;
    }
    let scaled_pos = vec2<f32>(quad_pos.x * w_eff, quad_pos.y * h_eff);

    // Face directions:
    // 0: PosX, 1: NegX, 2: PosY, 3: NegY, 4: PosZ, 5: NegZ
    if face == 0u { // PosX
        pos = vec3<f32>(x + 1.0, y + scaled_pos.x, z + scaled_pos.y);
    } else if face == 1u { // NegX
        pos = vec3<f32>(x, y + scaled_pos.y, z + scaled_pos.x);
    } else if face == 2u { // PosY
        pos = vec3<f32>(x + scaled_pos.y, y + 1.0, z + scaled_pos.x);
    } else if face == 3u { // NegY
        pos = vec3<f32>(x + scaled_pos.x, y, z + scaled_pos.y);
    } else if face == 4u { // PosZ
        pos = vec3<f32>(x + scaled_pos.x, y + scaled_pos.y, z + 1.0);
    } else { // NegZ (5)
        pos = vec3<f32>(x + scaled_pos.y, y + scaled_pos.x, z);
    }

    var out: VertexOutput;
    let world_pos = pos + chunk.world_pos;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    out.uv = scaled_pos;
    out.face = face;
    out.texture_id = u32(t);
    
    // Extract AO for this vertex
    let vertex_ao = (ao >> (2u * vertex.vertex_index)) & 0x3u;
    // Map 0-3 to 0.4-1.0
    out.ao = 0.4 + 0.6 * (f32(vertex_ao) / 3.0);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let texture_id = in.texture_id;
    let tex_dims = vec2<f32>(textureDimensions(t_diffuse));

    let tile_size = 16.0;
    let tiles_per_row = u32(tex_dims.x / tile_size);

    let tile_x = f32(texture_id % tiles_per_row);
    let tile_y = f32(texture_id / tiles_per_row);

    let tile_uv = fract(in.uv);
    let atlas_uv = (vec2<f32>(tile_x, tile_y) + tile_uv) * tile_size / tex_dims;

    let color = textureSample(t_diffuse, s_diffuse, atlas_uv);

    let final_color = color.rgb * in.ao;
    return vec4<f32>(final_color, 1.0);
}
