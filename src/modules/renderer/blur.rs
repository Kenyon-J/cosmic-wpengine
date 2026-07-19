//! Offscreen dual-Kawase blur matching COSMIC's native frosted-glass effect.
//!
//! cosmic-comp blurs the area behind frosted surfaces with a dual-Kawase
//! chain: N downsample passes (5 taps each, halving resolution every step)
//! followed by N upsample passes (8 taps each) back up, with the pass count
//! and sample offset taken from a 15-entry strength table. This module
//! reproduces that chain over the background source texture, but runs it
//! offscreen only when the artwork or the blur amount changes: mode 0 of
//! album_art.wgsl then mixes the cached result with the sharp image, so the
//! steady-state per-frame cost drops from 16 blur taps per pixel to one
//! extra texture fetch.

/// Deepest downsample level the chain allocates for. Matches the maximum
/// pass count in cosmic-comp's strength table.
const MAX_PASSES: usize = 4;

/// Sources larger than this are stepped down with extra downsample passes
/// before the Kawase chain proper. The blurred result has no fine detail to
/// preserve, and this caps the persistent VRAM of the cached output (a 4K
/// wallpaper would otherwise pin a full 33MB copy; capped it pins ~8MB).
const BASE_CAP: u32 = 2048;

/// cosmic-comp's `BLUR_PARAMS` table (src/backend/render/wayland/blur_effect.rs):
/// (passes, offset) for each of the theme's 15 `BlurStrength` levels, generated
/// there from the offset ranges 1-2 (1 pass), 2-3 (2), 2-5 (3) and 3-8 (4).
const BLUR_PARAMS: [(usize, f32); 15] = [
    (1, 1.0 + 1.0 / 3.0),
    (1, 1.0 + 2.0 / 3.0),
    (1, 2.0),
    (2, 2.0 + 1.0 / 3.0),
    (2, 2.0 + 2.0 / 3.0),
    (2, 3.0),
    (3, 2.0 + 3.0 / 7.0),
    (3, 2.0 + 6.0 / 7.0),
    (3, 2.0 + 9.0 / 7.0),
    (3, 2.0 + 12.0 / 7.0),
    (3, 2.0 + 15.0 / 7.0),
    (3, 2.0 + 18.0 / 7.0),
    (3, 5.0),
    (4, 5.5),
    (4, 8.0),
];

/// Maps the 0..1 "Blur Amount" slider onto cosmic-comp's strength table.
/// cosmic-comp indexes the table at `strength + 1`, so level 0 is unreachable
/// there; the slider covers the same 1..=14 range.
pub(crate) fn params_for_amount(amount: f32) -> (usize, f32) {
    let idx = 1 + (amount.clamp(0.0, 1.0) * 13.0).round() as usize;
    BLUR_PARAMS[idx.min(BLUR_PARAMS.len() - 1)]
}

/// The blur chain renders in the same format the source textures use
/// (album art and wallpapers are always uploaded as Rgba8UnormSrgb), so the
/// per-pass filtering happens in linear light like cosmic-comp's GL blur.
const CHAIN_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

pub(crate) struct KawaseBlur {
    pub(crate) layout: wgpu::BindGroupLayout,
    down_pipeline: wgpu::RenderPipeline,
    up_pipeline: wgpu::RenderPipeline,
}

impl KawaseBlur {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Kawase Blur Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Kawase Blur Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../kawase_blur.wgsl").into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Kawase Blur Pipeline Layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });

        let make_pipeline = |label: &str, entry_point: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry_point),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: CHAIN_FORMAT,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };

        Self {
            layout,
            down_pipeline: make_pipeline("Kawase Downsample Pipeline", "fs_down"),
            up_pipeline: make_pipeline("Kawase Upsample Pipeline", "fs_up"),
        }
    }
}

/// One level of the chain: a render target plus the bind group used by
/// whichever pass *reads* it (half_pixel in its uniform is 0.5 / this
/// level's size, exactly like cosmic-comp's per-pass `half_pixel`).
struct BlurLevel {
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    size: (u32, u32),
}

/// Persistent texture chain for one blur source (the album art or the custom
/// background). All levels are allocated up front for `MAX_PASSES` depth so
/// dragging the Blur Amount slider only re-runs passes, never reallocates.
pub(crate) struct BlurChain {
    /// Bind group reading the original source texture (first downsample).
    src_bind_group: wgpu::BindGroup,
    src_uniform_buffer: wgpu::Buffer,
    src_size: (u32, u32),
    /// `[prefilter levels.., base, base/2, base/4, base/8, base/16]` - the
    /// prefilter levels step oversized sources down to `BASE_CAP`.
    levels: Vec<BlurLevel>,
    /// Index of the base level in `levels`; it holds the finished blur.
    base: usize,
}

impl BlurChain {
    pub(crate) fn new(
        device: &wgpu::Device,
        blur: &KawaseBlur,
        sampler: &wgpu::Sampler,
        src_view: &wgpu::TextureView,
        src_size: (u32, u32),
        label: &str,
    ) -> Self {
        // Clamped at 1 so extreme aspect ratios (a panorama halving its
        // short side to zero mid-prefilter) can't request a zero-dimension
        // texture, which wgpu validation rejects.
        let halve = |(w, h): (u32, u32)| ((w / 2).max(1), (h / 2).max(1));

        // Level sizes: halvings of the source down to the capped base, then
        // MAX_PASSES further halvings for the Kawase chain itself. When the
        // source is already within the cap the base level keeps its size and
        // the first downsample reads the source directly (matching
        // cosmic-comp's full -> half first pass).
        let mut sizes = Vec::new();
        let mut cur = src_size;
        while cur.0.max(cur.1) > BASE_CAP {
            cur = halve(cur);
            sizes.push(cur);
        }
        let base = if sizes.is_empty() {
            sizes.push(src_size);
            0
        } else {
            sizes.len() - 1
        };
        for _ in 0..MAX_PASSES {
            cur = halve(cur);
            sizes.push(cur);
        }

        let make_uniform_and_bind_group = |view: &wgpu::TextureView| {
            let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(format!("{label} Blur Uniforms").as_str()),
                size: 16,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(format!("{label} Blur Bind Group").as_str()),
                layout: &blur.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: uniform_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });
            (uniform_buffer, bind_group)
        };

        let levels: Vec<BlurLevel> = sizes
            .iter()
            .map(|&size| {
                let texture = device.create_texture(&wgpu::TextureDescriptor {
                    label: Some(format!("{label} Blur Level {}x{}", size.0, size.1).as_str()),
                    size: wgpu::Extent3d {
                        width: size.0,
                        height: size.1,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: CHAIN_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::RENDER_ATTACHMENT,
                    view_formats: &[],
                });
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                let (uniform_buffer, bind_group) = make_uniform_and_bind_group(&view);
                BlurLevel {
                    view,
                    bind_group,
                    uniform_buffer,
                    size,
                }
            })
            .collect();

        let (src_uniform_buffer, src_bind_group) = make_uniform_and_bind_group(src_view);

        Self {
            src_bind_group,
            src_uniform_buffer,
            src_size,
            levels,
            base,
        }
    }

    /// View of the finished blur, for binding as `blur_tex` in album_art.wgsl.
    pub(crate) fn output_view(&self) -> &wgpu::TextureView {
        &self.levels[self.base].view
    }

    /// True if this chain was built for a source of `size`. A size match
    /// alone cannot prove the source is the same texture *object*: paths
    /// that recreate the source texture must drop the chain themselves
    /// (core/updates.rs does), because the chain binds the old texture's
    /// view and would keep blurring it. This check only spares a rebuild
    /// on the settings-change path, where the texture is untouched.
    pub(crate) fn matches_source(&self, size: (u32, u32)) -> bool {
        self.src_size == size
    }

    /// Encodes and submits the full blur: prefilter downsamples (oversized
    /// sources only), then the dual-Kawase chain at the strength mapped from
    /// `amount`, leaving the result in the base level.
    pub(crate) fn run(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        blur: &KawaseBlur,
        amount: f32,
    ) {
        let (passes, offset) = params_for_amount(amount);

        let write_uniforms = |buffer: &wgpu::Buffer, size: (u32, u32)| {
            let uniforms = [0.5 / size.0 as f32, 0.5 / size.1 as f32, offset, 0.0];
            queue.write_buffer(buffer, 0, bytemuck::cast_slice(&uniforms));
        };
        write_uniforms(&self.src_uniform_buffer, self.src_size);
        for level in &self.levels {
            write_uniforms(&level.uniform_buffer, level.size);
        }

        // Downsamples walk `levels[first_write..=base + passes]`; the source
        // read for each pass is the previous write (starting at the original
        // texture). When there are no prefilter levels the base level's size
        // equals the source's, so the first downsample skips it and writes
        // straight to base + 1; the base texture is then only written by the
        // final upsample.
        let first_write = if self.base == 0 && self.levels[0].size == self.src_size {
            1
        } else {
            0
        };
        let last_write = self.base + passes;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Kawase Blur Encoder"),
        });

        let mut encode_pass =
            |pipeline: &wgpu::RenderPipeline, read: &wgpu::BindGroup, write: &wgpu::TextureView| {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Kawase Blur Pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: write,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, read, &[]);
                pass.draw(0..3, 0..1);
            };

        let mut read = &self.src_bind_group;
        for i in first_write..=last_write {
            encode_pass(&blur.down_pipeline, read, &self.levels[i].view);
            read = &self.levels[i].bind_group;
        }
        for i in (self.base..last_write).rev() {
            encode_pass(
                &blur.up_pipeline,
                &self.levels[i + 1].bind_group,
                &self.levels[i].view,
            );
        }

        queue.submit(std::iter::once(encoder.finish()));
    }
}
