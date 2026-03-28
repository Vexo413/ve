struct InstanceInput {
    @location(0) data: u32,
};

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec3<f32>,
};

struct ChunkUniform {
    world_pos: vec3<f32>,
};

struct CameraUniform {
    view_proj: mat4x4<f32>,
};

@group(0) @binding(0)
var<uniform> chunk: ChunkUniform;
@group(1) @binding(0)
var<uniform> camera: CameraUniform;

@vertex
fn vs_main(
    instance: InstanceInput,
    vertex: VertexInput,
) -> VertexOutput {
    let data = instance.data;
    
    // Unpack data
    // WWWWWHHHHHTTTTFFFZZZZZYYYYYXXXXX
    // X: 0-4 (5 bits)
    // Y: 5-9 (5 bits)
    // Z: 10-14 (5 bits)
    // F: 15-17 (3 bits)
    // T: 18-21 (4 bits)
    // H: 22-26 (5 bits)
    // W: 27-31 (5 bits)

    let x = f32(data & 0x1Fu);
    let y = f32((data >> 5u) & 0x1Fu);
    let z = f32((data >> 10u) & 0x1Fu);
    let face = (data >> 15u) & 0x7u;
    let block_type = (data >> 18u) & 0xFu;
    let h = f32((data >> 22u) & 0x1Fu);
    let w = f32((data >> 27u) & 0x1Fu);

    var pos: vec3<f32>;
    let quad_pos = vec2<f32>(
        f32(vertex.vertex_index % 2u),
        f32(vertex.vertex_index / 2u)
    );
    
    // quad_pos is (0,0), (1,0), (0,1), (1,1)
    // We want to scale it by w and h
    let scaled_pos = vec2<f32>(quad_pos.x * w, quad_pos.y * h);

    // Face directions:
    // 0: PosX, 1: NegX, 2: PosY, 3: NegY, 4: PosZ, 5: NegZ
    if face == 0u { // PosX
        pos = vec3<f32>(x, y + scaled_pos.x, z + scaled_pos.y);
    } else if face == 1u { // NegX
        pos = vec3<f32>(x, y + scaled_pos.y, z + scaled_pos.x);
    } else if face == 2u { // PosY
        pos = vec3<f32>(x + scaled_pos.y, y, z + scaled_pos.x);
    } else if face == 3u { // NegY
        pos = vec3<f32>(x + scaled_pos.x, y, z + scaled_pos.y);
    } else if face == 4u { // PosZ
        pos = vec3<f32>(x + scaled_pos.x, y + scaled_pos.y, z);
    } else { // NegZ (5)
        pos = vec3<f32>(x + scaled_pos.y, y + scaled_pos.x, z);
    }

    var out: VertexOutput;
    let world_pos = pos + chunk.world_pos;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);
    
    // Vary color by face and block type
    let colors = array<vec3<f32>, 6>(
        vec3<f32>(1.0, 0.0, 0.0),
        vec3<f32>(1.0, 1.0, 0.0),
        vec3<f32>(0.0, 1.0, 0.0),
        vec3<f32>(0.0, 1.0, 1.0),
        vec3<f32>(0.0, 0.0, 1.0),
        vec3<f32>(1.0, 1.0, 1.0)
    );
    out.color = colors[face];
    if block_type == 1u { // Dirt
        out.color     *= 0.5;
    }

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color, 1.0);
}
