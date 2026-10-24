use crate::gpu::context::GpuContext;
use glam::Mat4;

// Bright red dots on the centers of the selected splats, drawn on top of the scene.
pub struct SelectionPass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params_buffer: wgpu::Buffer,
    positions: Option<wgpu::Buffer>,
    count: u32,
}

impl SelectionPass {
    pub fn new(gpu: &GpuContext) -> Self {
        let params_buffer = gpu.create_buffer(
            "Selection Params",
            80,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let bind_group_layout = gpu.create_bind_group_layout(
            "selection bind group layout",
            &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        );
        let bind_group = gpu.create_bind_group(
            "selection bind group",
            &bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: params_buffer.as_entire_binding(),
            }],
        );

        let pipeline_layout =
            gpu.create_pipeline_layout("Selection Pipeline Layout", &[Some(&bind_group_layout)]);
        let shader = gpu.create_shader_module(wgpu::include_wgsl!("../shaders/selection.wgsl"));
        let attributes = wgpu::vertex_attr_array![0 => Float32x3];
        let pipeline = gpu.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 12,
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &attributes,
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: gpu.surface_format(),
                    blend: None,
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
            positions: None,
            count: 0,
        }
    }

    // Positions are in the scene's own (file) space.
    pub fn set_positions(&mut self, gpu: &GpuContext, positions: &[[f32; 3]]) {
        self.count = positions.len() as u32;
        self.positions = (!positions.is_empty()).then(|| {
            gpu.create_buffer_init(
                "Selection Positions",
                positions,
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            )
        });
    }

    pub fn update(&self, gpu: &GpuContext, view_proj: Mat4) {
        let surface = gpu.surface_size();
        let mut data = [0.0f32; 20];
        data[..16].copy_from_slice(&view_proj.to_cols_array());
        data[16] = surface.width.max(1) as f32;
        data[17] = surface.height.max(1) as f32;
        gpu.write_buffer(&self.params_buffer, 0, &data);
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, output_view: &wgpu::TextureView) {
        let Some(positions) = &self.positions else {
            return;
        };

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Selection Pass"),
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
        pass.set_vertex_buffer(0, positions.slice(..));
        pass.draw(0..6, 0..self.count);
    }
}
