use super::text_overlay::{FONT, GLYPH_HEIGHT, GLYPH_WIDTH};
use crate::gpu::context::GpuContext;

const CELL: usize = 8;
const ATLAS_COLUMNS: usize = 16;
const ATLAS_ROWS: usize = 6;
const ATLAS_WIDTH: usize = CELL * ATLAS_COLUMNS;
const ATLAS_HEIGHT: usize = CELL * ATLAS_ROWS;
// The last atlas cell is fully lit and used for filled rectangles.
const SOLID_CELL: usize = 95;
const MAX_VERTICES: usize = 16200;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}

// Rectangles and text in window pixels, collected each frame and drawn in one pass.
#[derive(Default)]
pub struct UiBatch {
    vertices: Vec<UiVertex>,
}

impl UiBatch {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, uv: [f32; 4], color: [f32; 4]) {
        let corner = |px: f32, py: f32, u: f32, v: f32| UiVertex {
            position: [px, py],
            uv: [u, v],
            color,
        };
        let [u0, v0, u1, v1] = uv;
        let (x1, y1) = (x + w, y + h);

        self.vertices.extend_from_slice(&[
            corner(x, y, u0, v0),
            corner(x1, y, u1, v0),
            corner(x, y1, u0, v1),
            corner(x, y1, u0, v1),
            corner(x1, y, u1, v0),
            corner(x1, y1, u1, v1),
        ]);
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        let (cx, cy) = (SOLID_CELL % ATLAS_COLUMNS, SOLID_CELL / ATLAS_COLUMNS);
        let u = (cx * CELL + CELL / 2) as f32 / ATLAS_WIDTH as f32;
        let v = (cy * CELL + CELL / 2) as f32 / ATLAS_HEIGHT as f32;
        self.quad(x, y, w, h, [u, v, u, v], color);
    }

    // `scale` should be a whole number so the pixel font stays crisp.
    pub fn text(&mut self, x: f32, y: f32, scale: f32, text: &str, color: [f32; 4]) {
        let mut pen = x;
        for c in text.chars() {
            let index = (c as usize).wrapping_sub(0x20);
            if c != ' ' && index < 95 {
                let (cx, cy) = (index % ATLAS_COLUMNS, index / ATLAS_COLUMNS);
                let uv = [
                    (cx * CELL) as f32 / ATLAS_WIDTH as f32,
                    (cy * CELL) as f32 / ATLAS_HEIGHT as f32,
                    (cx * CELL + GLYPH_WIDTH) as f32 / ATLAS_WIDTH as f32,
                    (cy * CELL + GLYPH_HEIGHT) as f32 / ATLAS_HEIGHT as f32,
                ];
                self.quad(
                    pen,
                    y,
                    GLYPH_WIDTH as f32 * scale,
                    GLYPH_HEIGHT as f32 * scale,
                    uv,
                    color,
                );
            }
            pen += (GLYPH_WIDTH + 1) as f32 * scale;
        }
    }

    pub fn text_width(text: &str, scale: f32) -> f32 {
        (text.chars().count() as f32 * (GLYPH_WIDTH + 1) as f32 - 1.0).max(0.0) * scale
    }
}

fn build_atlas() -> Vec<u8> {
    let mut pixels = vec![0u8; ATLAS_WIDTH * ATLAS_HEIGHT];

    for (index, glyph) in FONT.iter().enumerate() {
        let (cx, cy) = (index % ATLAS_COLUMNS, index / ATLAS_COLUMNS);
        for (col, bits) in glyph.iter().enumerate() {
            for row in 0..GLYPH_HEIGHT {
                if (bits >> row) & 1 == 1 {
                    pixels[(cy * CELL + row) * ATLAS_WIDTH + cx * CELL + col] = 255;
                }
            }
        }
    }

    let (cx, cy) = (SOLID_CELL % ATLAS_COLUMNS, SOLID_CELL / ATLAS_COLUMNS);
    for row in 0..CELL {
        for col in 0..CELL {
            pixels[(cy * CELL + row) * ATLAS_WIDTH + cx * CELL + col] = 255;
        }
    }

    pixels
}

pub struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    vertex_buffer: wgpu::Buffer,
}

impl UiRenderer {
    pub fn new(gpu: &GpuContext) -> Self {
        let texture = gpu.create_texture(&wgpu::TextureDescriptor {
            label: Some("UI Atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_WIDTH as u32,
                height: ATLAS_HEIGHT as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        gpu.write_texture_r8(
            &texture,
            &build_atlas(),
            ATLAS_WIDTH as u32,
            ATLAS_HEIGHT as u32,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let sampler = gpu.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("UI Sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let params_buffer = gpu.create_buffer(
            "UI Params",
            16,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );
        let vertex_buffer = gpu.create_buffer(
            "UI Vertices",
            (MAX_VERTICES * std::mem::size_of::<UiVertex>()) as u64,
            wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        );

        let bind_group_layout = gpu.create_bind_group_layout(
            "ui bind group layout",
            &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        );
        let bind_group = gpu.create_bind_group(
            "ui bind group",
            &bind_group_layout,
            &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        );

        let pipeline_layout =
            gpu.create_pipeline_layout("UI Pipeline Layout", &[Some(&bind_group_layout)]);
        let shader = gpu.create_shader_module(wgpu::include_wgsl!("../shaders/ui.wgsl"));
        let attributes = wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
            2 => Float32x4,
        ];
        let pipeline = gpu.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<UiVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.surface_format(),
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        Self {
            pipeline,
            bind_group,
            params_buffer,
            vertex_buffer,
        }
    }

    pub fn encode(
        &self,
        gpu: &GpuContext,
        encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        batch: &UiBatch,
    ) {
        if batch.is_empty() {
            return;
        }

        let count = batch.vertices.len().min(MAX_VERTICES);
        let surface = gpu.surface_size();
        gpu.write_buffer(
            &self.params_buffer,
            0,
            &[
                surface.width.max(1) as f32,
                surface.height.max(1) as f32,
                0.0,
                0.0,
            ],
        );
        gpu.write_buffer(&self.vertex_buffer, 0, &batch.vertices[..count]);

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("UI Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        pass.draw(0..count as u32, 0..1);
    }
}
