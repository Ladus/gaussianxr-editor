use crate::gpu::context::GpuContext;
use glam::{Mat4, Vec3};

// Line and dots for the two measurement points, drawn on top of the scene.
pub struct MeasurePass {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    buffer: wgpu::Buffer,
    visible: bool,
}

impl MeasurePass {
    pub fn new(gpu: &GpuContext) -> Self {
        let buffer = gpu.create_buffer(
            "Measure Params",
            112,
            wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        );

        let bind_group_layout = gpu.create_bind_group_layout(
            "measure bind group layout",
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
            "measure bind group",
            &bind_group_layout,
            &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        );

        let pipeline_layout =
            gpu.create_pipeline_layout("Measure Pipeline Layout", &[Some(&bind_group_layout)]);
        let shader = gpu.create_shader_module(wgpu::include_wgsl!("../shaders/measure.wgsl"));
        let pipeline = gpu.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("measure pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
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
            buffer,
            visible: false,
        }
    }

    pub fn update(
        &mut self,
        gpu: &GpuContext,
        view_proj: Mat4,
        a: Option<Vec3>,
        b: Option<Vec3>,
    ) {
        let point = |p: Option<Vec3>| {
            let v = p.unwrap_or(Vec3::ZERO);
            [v.x, v.y, v.z, if p.is_some() { 1.0 } else { 0.0 }]
        };

        let surface = gpu.surface_size();
        let mut data = [0.0f32; 28];
        data[..16].copy_from_slice(&view_proj.to_cols_array());
        data[16..20].copy_from_slice(&point(a));
        data[20..24].copy_from_slice(&point(b));
        data[24] = surface.width.max(1) as f32;
        data[25] = surface.height.max(1) as f32;
        gpu.write_buffer(&self.buffer, 0, &data);

        self.visible = a.is_some() || b.is_some();
    }

    pub fn encode(&self, encoder: &mut wgpu::CommandEncoder, output_view: &wgpu::TextureView) {
        if !self.visible {
            return;
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Measure Pass"),
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
        pass.draw(0..6, 0..3);
    }
}
