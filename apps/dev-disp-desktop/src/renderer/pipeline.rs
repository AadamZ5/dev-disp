//! AI Generated video pipeline code
//! 
//! LAYER 3: `Pipeline` — created once (first time this `Primitive` type is
//! seen) and persists across every frame in the renderer's `Storage`.
//! Responsibility: own the actual wgpu resources (texture, sampler, render
//! pipeline). This is where "the video frame" actually lives on the GPU.

use iced::wgpu;
use iced::widget::shader;
use iced::Rectangle;

/// Fill color shown before the first frame arrives (a neutral "no signal" gray).
const PLACEHOLDER_RGBA: [u8; 4] = [20, 20, 20, 255];

pub struct VideoPipeline {
    render_pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    /// Always `Some` after construction - starts as a 1x1 placeholder so the
    /// widget always has something to draw instead of leaving the surface untouched.
    texture: Option<TextureSlot>,
    /// Dimensions of the last uploaded frame, used by `refresh_scale` to
    /// recompute letterboxing on resize without needing new pixel data.
    last_frame_size: (u32, u32),
}

struct TextureSlot {
    width: u32,
    height: u32,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

impl shader::Pipeline for VideoPipeline {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("video_feed.shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("video_feed.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("video_feed.bind_group_layout"),
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
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("video_feed.pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("video_feed.pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                // Fullscreen triangle is generated from `vertex_index`; no vertex buffer needed.
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("video_feed.sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("video_feed.uniforms"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut pipeline = Self {
            render_pipeline,
            bind_group_layout,
            sampler,
            uniform_buffer,
            texture: None,
            last_frame_size: (1, 1),
        };

        // Eagerly fill in a 1x1 placeholder so there's always something valid to
        // draw, rather than leaving the widget's area untouched before any data arrives.
        let slot = pipeline.create_texture_slot(device, 1, 1);
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &slot.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &PLACEHOLDER_RGBA,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        );
        pipeline.texture = Some(slot);
        pipeline.write_scale(queue, [1.0, 1.0]);

        pipeline
    }
}

impl VideoPipeline {
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
        bounds: Rectangle,
        letterbox: bool,
    ) {
        let needs_recreate =
            !matches!(&self.texture, Some(slot) if slot.width == width && slot.height == height);

        if needs_recreate {
            self.texture = Some(self.create_texture_slot(device, width, height));
        }

        self.last_frame_size = (width, height);
        self.write_scale(queue, compute_scale(bounds, width, height, letterbox));

        let slot = self.texture.as_ref().expect("just created above");
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &slot.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
    }

    fn create_texture_slot(&self, device: &wgpu::Device, width: u32, height: u32) -> TextureSlot {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("video_feed.texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // Plain (non-sRGB) format: the decoded RGBA bytes are used as-is, matching
            // how `widget::image` renders raw pixel data. Using an `*Srgb` format here
            // applies an extra gamma decode on sample, which is what caused the darkening.
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("video_feed.bind_group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: self.uniform_buffer.as_entire_binding(),
                },
            ],
        });

        TextureSlot {
            width,
            height,
            texture,
            bind_group,
        }
    }

    /// Resets the scale to fill the bounds completely, so whatever texture is
    /// currently bound (the placeholder, if no frame has arrived yet) fills the
    /// widget instead of leaving its area untouched/transparent.
    pub fn show_placeholder(&self, queue: &wgpu::Queue) {
        self.write_scale(queue, [1.0, 1.0]);
    }

    /// Recomputes letterbox scale against the last uploaded frame's dimensions,
    /// without touching the texture. Used when no new frame arrived this tick
    /// (e.g. the window was resized but the decoder hasn't produced a frame yet).
    pub fn refresh_scale(&self, queue: &wgpu::Queue, bounds: Rectangle, letterbox: bool) {
        let (width, height) = self.last_frame_size;
        self.write_scale(queue, compute_scale(bounds, width, height, letterbox));
    }

    fn write_scale(&self, queue: &wgpu::Queue, scale: [f32; 2]) {
        let mut bytes = [0u8; 16];
        bytes[0..4].copy_from_slice(&scale[0].to_ne_bytes());
        bytes[4..8].copy_from_slice(&scale[1].to_ne_bytes());
        queue.write_buffer(&self.uniform_buffer, 0, &bytes);
    }

    pub fn draw(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clip_bounds: Rectangle<u32>,
    ) {
        // Always `Some`: a 1x1 placeholder is created eagerly in `new()`.
        let slot = self.texture.as_ref().expect("texture always initialized");

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("video_feed.pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        pass.set_viewport(
            clip_bounds.x as f32,
            clip_bounds.y as f32,
            clip_bounds.width as f32,
            clip_bounds.height as f32,
            0.0,
            1.0,
        );
        pass.set_pipeline(&self.render_pipeline);
        pass.set_bind_group(0, &slot.bind_group, &[]);
        pass.draw(0..6, 0..1);
    }
}

/// Computes the normalized scale to apply to the full-bounds quad so the
/// frame's aspect ratio is preserved (letterbox/pillarbox) instead of stretched.
fn compute_scale(bounds: Rectangle, frame_width: u32, frame_height: u32, letterbox: bool) -> [f32; 2] {
    if !letterbox || bounds.width <= 0.0 || bounds.height <= 0.0 || frame_width == 0 || frame_height == 0 {
        return [1.0, 1.0];
    }

    let frame_aspect = frame_width as f32 / frame_height as f32;
    let bounds_aspect = bounds.width / bounds.height;

    if frame_aspect > bounds_aspect {
        // Frame is relatively wider than the box: shrink vertically (letterbox top/bottom).
        [1.0, bounds_aspect / frame_aspect]
    } else {
        // Frame is relatively taller than the box: shrink horizontally (pillarbox sides).
        [frame_aspect / bounds_aspect, 1.0]
    }
}
