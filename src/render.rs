use crate::{
    chunk::mesh,
    constants::{CHUNK_SIZE, RENDER_DISTANCE},
    position::IVec3,
    world::World,
};
use std::{sync::Arc, time::Instant};

use ahash::AHashMap;
use cgmath::Vector3;
use image::GenericImageView;
use wgpu::util::DeviceExt;
use winit::{
    application::ApplicationHandler,
    event::WindowEvent,
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle},
    window::{Window, WindowId},
};

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkUniform {
    world_pos: [f32; 3],
    _padding: f32,
}

struct Camera {
    eye: cgmath::Point3<f32>,
    target: cgmath::Point3<f32>,
    up: cgmath::Vector3<f32>,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl Camera {
    fn build_view_projection_matrix(&self) -> cgmath::Matrix4<f32> {
        let view = cgmath::Matrix4::look_at_rh(self.eye, self.target, self.up);
        let proj = cgmath::perspective(cgmath::Deg(self.fovy), self.aspect, self.znear, self.zfar);
        return OPENGL_TO_WGPU_MATRIX * proj * view;
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    fn new() -> Self {
        use cgmath::SquareMatrix;
        Self {
            view_proj: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_view_proj(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_projection_matrix().into();
    }
}

pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.0, 0.0, 0.0, 1.0,
);

struct CameraController {
    speed: f32,
    sensitivity: f32,
    is_down_pressed: bool,
    is_up_pressed: bool,
    is_forward_pressed: bool,
    is_backward_pressed: bool,
    is_left_pressed: bool,
    is_right_pressed: bool,
    yaw: f32,
    pitch: f32,
    cursor_locked: bool,
}

impl CameraController {
    fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            speed,
            sensitivity,
            is_down_pressed: false,
            is_up_pressed: false,
            is_forward_pressed: false,
            is_backward_pressed: false,
            is_left_pressed: false,
            is_right_pressed: false,
            yaw: -90.0,
            pitch: 0.0,
            cursor_locked: false,
        }
    }

    fn process_events(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state,
                        physical_key: winit::keyboard::PhysicalKey::Code(key),
                        ..
                    },
                ..
            } => {
                let is_pressed = *state == winit::event::ElementState::Pressed;
                match key {
                    winit::keyboard::KeyCode::ShiftLeft => {
                        self.is_down_pressed = is_pressed;
                        true
                    }
                    winit::keyboard::KeyCode::Space => {
                        self.is_up_pressed = is_pressed;
                        true
                    }
                    winit::keyboard::KeyCode::KeyW | winit::keyboard::KeyCode::ArrowUp => {
                        self.is_forward_pressed = is_pressed;
                        true
                    }
                    winit::keyboard::KeyCode::KeyA | winit::keyboard::KeyCode::ArrowLeft => {
                        self.is_left_pressed = is_pressed;
                        true
                    }
                    winit::keyboard::KeyCode::KeyS | winit::keyboard::KeyCode::ArrowDown => {
                        self.is_backward_pressed = is_pressed;
                        true
                    }
                    winit::keyboard::KeyCode::KeyD | winit::keyboard::KeyCode::ArrowRight => {
                        self.is_right_pressed = is_pressed;
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        if self.cursor_locked {
            self.yaw += mouse_dx as f32 * self.sensitivity;
            self.pitch -= mouse_dy as f32 * self.sensitivity;
        }

        if self.pitch > 89.0 {
            self.pitch = 89.0;
        } else if self.pitch < -89.0 {
            self.pitch = -89.0;
        }
    }

    fn update_camera(&self, camera: &mut Camera, dt: f32) {
        use cgmath::InnerSpace;

        let forward = cgmath::Vector3::new(
            self.yaw.to_radians().cos() * self.pitch.to_radians().cos(),
            self.pitch.to_radians().sin(),
            self.yaw.to_radians().sin() * self.pitch.to_radians().cos(),
        )
        .normalize();

        camera.target = camera.eye + forward;

        let forward = Vector3::new(forward.x, 0.0, forward.z);

        let right = forward.cross(camera.up).normalize();

        if self.is_down_pressed {
            camera.eye.y -= self.speed * dt;
            camera.target.y -= self.speed * dt;
        }
        if self.is_up_pressed {
            camera.eye.y += self.speed * dt;
            camera.target.y += self.speed * dt;
        }
        if self.is_forward_pressed {
            camera.eye += forward * (self.speed * dt);
            camera.target += forward * (self.speed * dt);
        }
        if self.is_backward_pressed {
            camera.eye -= forward * (self.speed * dt);
            camera.target -= forward * (self.speed * dt);
        }
        if self.is_right_pressed {
            camera.eye += right * (self.speed * dt);
            camera.target += right * (self.speed * dt);
        }
        if self.is_left_pressed {
            camera.eye -= right * (self.speed * dt);
            camera.target -= right * (self.speed * dt);
        }
    }
}

struct ChunkRenderData {
    face_buffers: [Option<wgpu::Buffer>; 6],
    face_counts: [u32; 6],
    bind_group: wgpu::BindGroup,
}

struct State {
    instance: wgpu::Instance,
    window: Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    size: winit::dpi::PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    surface_format: wgpu::TextureFormat,
    render_pipeline: wgpu::RenderPipeline,
    depth_texture_view: wgpu::TextureView,
    index_buffer: wgpu::Buffer,
    chunk_bind_group_layout: wgpu::BindGroupLayout,
    chunks: AHashMap<IVec3, Option<ChunkRenderData>>,
    world: World,
    camera: Camera,
    camera_controller: CameraController,
    camera_uniform: CameraUniform,
    camera_buffer: wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,
    fps_timer: Instant,
    last_frame_instant: Instant,
    frame_count: u32,
    fps: u32,
    atlas_bind_group: wgpu::BindGroup,
}

impl State {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    fn create_depth_texture(
        device: &wgpu::Device,
        size: &winit::dpi::PhysicalSize<u32>,
        label: &str,
    ) -> wgpu::TextureView {
        let size = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };
        let desc = wgpu::TextureDescriptor {
            label: Some(label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Self::DEPTH_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        };
        let texture = device.create_texture(&desc);
        texture.create_view(&wgpu::TextureViewDescriptor::default())
    }

    fn create_atlas_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        label: &str,
    ) -> (wgpu::TextureView, wgpu::Sampler) {
        let atlas_bytes = include_bytes!("atlas.png");
        let atlas_image = image::load_from_memory(atlas_bytes).unwrap();
        let atlas_rgba = atlas_image.to_rgba8();
        let (width, height) = atlas_image.dimensions();

        let texture_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas_rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            texture_size,
        );
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("atlas_sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        (texture_view, sampler)
    }

    async fn new(display: OwnedDisplayHandle, window: Arc<Window>) -> State {
        // SETUP
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_with_display_handle(
            Box::new(display),
        ));
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .unwrap();
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .unwrap();

        let size = window.inner_size();

        let surface = instance.create_surface(window.clone()).unwrap();
        let cap = surface.get_capabilities(&adapter);
        let surface_format = cap.formats[0];

        // CHUNK
        let chunk_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Chunk Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // CAMERA
        let camera = Camera {
            eye: (-10.0, -10.0, -10.0).into(),
            target: (0.0, 0.0, 0.0).into(),
            up: cgmath::Vector3::unit_y(),
            aspect: size.width as f32 / size.height as f32,
            fovy: 45.0,
            znear: 0.1,
            zfar: 100.0,
        };

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("camera_bind_group_layout"),
            });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
            label: Some("camera_bind_group"),
        });
        // TEXTURES
        let (atlas_texture_view, atlas_sampler) =
            Self::create_atlas_texture(&device, &queue, "Atlas Texture");
        let atlas_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Atlas Bind Group Layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Atlas Bind Group"),
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        // PIPELINE
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Voxel Pipeline Layout"),
            bind_group_layouts: &[
                Some(&chunk_bind_group_layout),
                Some(&camera_bind_group_layout),
                Some(&atlas_bind_group_layout),
            ],
            immediate_size: 0,
        });

        let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Voxel Shader Module"),
            source: wgpu::ShaderSource::Wgsl(include_str!("main.wgsl").into()),
        });

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<u32>() as u64,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint32,
                offset: 0,
                shader_location: 0,
            }],
            step_mode: wgpu::VertexStepMode::Instance,
        };

        let depth_texture_view = Self::create_depth_texture(&device, &size, "depth_texture");

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Voxel Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[vertex_buffer_layout],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: Self::DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(surface_format.into())],
            }),
            multiview_mask: None,
            cache: None,
        });

        // Indices
        let indices: &[u16] = &[0, 1, 2, 1, 3, 2];
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Index Buffer"),
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        // WORLD
        let mut world = World::new(RENDER_DISTANCE);
        world.update_load_area(IVec3::new(0, 0, 0));

        let state = State {
            instance,
            window,
            device,
            queue,
            size,
            surface,
            surface_format,
            render_pipeline,
            depth_texture_view,
            index_buffer,
            chunk_bind_group_layout,
            chunks: AHashMap::new(),
            world,
            camera,
            camera_controller: CameraController::new(10.0, 0.1),
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            fps_timer: Instant::now(),
            last_frame_instant: Instant::now(),
            frame_count: 0,
            fps: 0,
            atlas_bind_group,
        };

        state.configure_surface();
        state
    }

    fn get_window(&self) -> &Window {
        &self.window
    }

    fn sync_world_to_gpu(&mut self) {
        // Process any newly generated chunks
        self.world.process_responses();

        // Update world based on camera position
        let camera_pos = IVec3::new(
            (self.camera.eye.x / CHUNK_SIZE as f32).floor() as i32,
            (self.camera.eye.y / CHUNK_SIZE as f32).floor() as i32,
            (self.camera.eye.z / CHUNK_SIZE as f32).floor() as i32,
        );
        self.world.update_load_area(camera_pos);

        // Remove chunks that are no longer in the world
        self.chunks
            .retain(|pos, _| self.world.chunks.contains_key(pos));

        // Add new chunks
        let range = -RENDER_DISTANCE..=RENDER_DISTANCE;

        for x in range.clone() {
            for y in range.clone() {
                for z in range.clone() {
                    let pos = camera_pos + IVec3::new(x, y, z);

                    // Skip chunks we already have
                    if self.chunks.contains_key(&pos) {
                        continue;
                    }

                    // Skip if no chunk refs
                    let refs = match self.world.get_chunk_refs(pos) {
                        Some(r) => r,
                        None => continue,
                    };

                    let instances = mesh(refs);

                    // Skip empty chunks
                    if instances.iter().all(|v| v.is_empty()) {
                        self.chunks.insert(pos, None);
                        continue;
                    }

                    let mut face_buffers: [Option<wgpu::Buffer>; 6] = Default::default();
                    let mut face_counts: [u32; 6] = [0; 6];

                    for (f, inst) in instances.iter().enumerate() {
                        if !inst.is_empty() {
                            face_buffers[f] = Some(self.device.create_buffer_init(
                                &wgpu::util::BufferInitDescriptor {
                                    label: Some("Instance Buffer"),
                                    contents: bytemuck::cast_slice(inst),
                                    usage: wgpu::BufferUsages::VERTEX,
                                },
                            ));
                            face_counts[f] = inst.len() as u32;
                        }
                    }

                    let pos_world = [
                        (pos.x * CHUNK_SIZE as i32) as f32,
                        (pos.y * CHUNK_SIZE as i32) as f32,
                        (pos.z * CHUNK_SIZE as i32) as f32,
                    ];

                    let uniform = ChunkUniform {
                        world_pos: pos_world,
                        _padding: 0.0,
                    };

                    let uniform_buffer =
                        self.device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Chunk Uniform Buffer"),
                                contents: bytemuck::cast_slice(&[uniform]),
                                usage: wgpu::BufferUsages::UNIFORM,
                            });

                    let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Chunk Bind Group"),
                        layout: &self.chunk_bind_group_layout,
                        entries: &[wgpu::BindGroupEntry {
                            binding: 0,
                            resource: uniform_buffer.as_entire_binding(),
                        }],
                    });

                    self.chunks.insert(
                        pos,
                        Some(ChunkRenderData {
                            face_buffers,
                            face_counts,
                            bind_group,
                        }),
                    );
                }
            }
        }
    }

    fn configure_surface(&self) {
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: self.surface_format,
            view_formats: vec![self.surface_format.add_srgb_suffix()],
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            width: self.size.width,
            height: self.size.height,
            desired_maximum_frame_latency: 2,
            present_mode: wgpu::PresentMode::AutoVsync,
        };
        self.surface.configure(&self.device, &surface_config);
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.configure_surface();
            self.depth_texture_view =
                Self::create_depth_texture(&self.device, &self.size, "depth_texture");
        }
    }

    fn update(&mut self) {
        let dt = self.last_frame_instant.elapsed().as_secs_f32();
        self.last_frame_instant = Instant::now();
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[self.camera_uniform]),
        );
        self.sync_world_to_gpu();
    }

    fn render(&mut self) {
        let surface_texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture) => texture,
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => return,
            wgpu::CurrentSurfaceTexture::Suboptimal(_) | wgpu::CurrentSurfaceTexture::Outdated => {
                self.configure_surface();
                return;
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                unreachable!("No error scope registered, so validation errors will panic")
            }
            wgpu::CurrentSurfaceTexture::Lost => {
                self.surface = self.instance.create_surface(self.window.clone()).unwrap();
                self.configure_surface();
                return;
            }
        };
        let texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor {
                format: Some(self.surface_format.add_srgb_suffix()),
                ..Default::default()
            });

        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Voxel Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.529,
                            g: 0.808,
                            b: 0.922,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(1, &self.camera_bind_group, &[]);
            render_pass.set_bind_group(2, &self.atlas_bind_group, &[]);
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            let cam_x = self.camera.eye.x;
            let cam_y = self.camera.eye.y;
            let cam_z = self.camera.eye.z;

            for (pos, chunk_data) in self.chunks.iter() {
                if let Some(chunk_data) = chunk_data {
                    render_pass.set_bind_group(0, &chunk_data.bind_group, &[]);

                    let min_x = (pos.x * CHUNK_SIZE as i32) as f32;
                    let max_x = min_x + CHUNK_SIZE as f32;
                    let min_y = (pos.y * CHUNK_SIZE as i32) as f32;
                    let max_y = min_y + CHUNK_SIZE as f32;
                    let min_z = (pos.z * CHUNK_SIZE as i32) as f32;
                    let max_z = min_z + CHUNK_SIZE as f32;

                    for f in 0..6 {
                        if let Some(buffer) = &chunk_data.face_buffers[f] {
                            let visible = match f {
                                0 => cam_x > min_x, // PosX faces point to +X, visible if cam_x > face_x
                                1 => cam_x < max_x, // NegX faces point to -X, visible if cam_x < face_x
                                2 => cam_y > min_y, // PosY faces point to +Y, visible if cam_y > face_y
                                3 => cam_y < max_y, // NegY faces point to -Y, visible if cam_y < face_y
                                4 => cam_z > min_z, // PosZ faces point to +Z, visible if cam_z > face_z
                                5 => cam_z < max_z, // NegZ faces point to -Z, visible if cam_z < face_z
                                _ => true,
                            };

                            if visible {
                                // println!("We are drawing");
                                render_pass.set_vertex_buffer(0, buffer.slice(..));
                                render_pass.draw_indexed(0..6, 0, 0..chunk_data.face_counts[f]);
                            }
                        }
                    }
                }
            }
        }

        self.queue.submit([encoder.finish()]);
        self.window.pre_present_notify();
        surface_texture.present();

        self.frame_count += 1;
        let elapsed = self.fps_timer.elapsed().as_secs_f64();
        if elapsed >= 1.0 {
            self.fps = self.frame_count;
            self.frame_count = 0;
            self.fps_timer = Instant::now();
            self.window.set_title(&format!("ve - {} FPS", self.fps));
        }
    }
}

#[derive(Default)]
struct App {
    state: Option<State>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window = Arc::new(
            event_loop
                .create_window(Window::default_attributes())
                .unwrap(),
        );

        let state = pollster::block_on(State::new(
            event_loop.owned_display_handle(),
            window.clone(),
        ));

        self.state = Some(state);
        window.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();
        if state.camera_controller.process_events(&event) {
            return;
        }
        match event {
            WindowEvent::MouseInput {
                state: button_state,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                if button_state == winit::event::ElementState::Pressed {
                    let _ = state
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::Locked);
                    state.window.set_cursor_visible(false);
                    state.camera_controller.cursor_locked = true;
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state: key_state,
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape),
                        ..
                    },
                ..
            } => {
                if key_state == winit::event::ElementState::Pressed {
                    let _ = state
                        .window
                        .set_cursor_grab(winit::window::CursorGrabMode::None);
                    state.window.set_cursor_visible(true);
                    state.camera_controller.cursor_locked = false;
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state: key_state,
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit1),
                        ..
                    },
                ..
            } => {
                if key_state == winit::event::ElementState::Pressed {
                    state.camera_controller.speed = 10.0;
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        state: key_state,
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit2),
                        ..
                    },
                ..
            } => {
                if key_state == winit::event::ElementState::Pressed {
                    state.camera_controller.speed = 100.0;
                }
            }
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                state.update();
                state.render();
                state.get_window().request_redraw();
            }
            WindowEvent::Resized(size) => {
                state.resize(size);
                state.camera.aspect = size.width as f32 / size.height as f32;
            }
            _ => (),
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let winit::event::DeviceEvent::MouseMotion { delta } = event {
            if let Some(state) = self.state.as_mut() {
                state.camera_controller.process_mouse(delta.0, delta.1);
            }
        }
    }
}

pub fn start() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::default();
    event_loop.run_app(&mut app).unwrap();
}
