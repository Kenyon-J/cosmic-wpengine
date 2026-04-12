const TEXT_SHADER_SRC: &str = include_str!("../text.wgsl");
use super::core::{GLYPH_CACHE_HEIGHT, GLYPH_CACHE_WIDTH};

use anyhow::Result;
use cosmic_text::{self, Buffer, FontSystem, SwashCache};

#[derive(PartialEq, Eq, Hash, Clone, Copy, Debug)]
pub enum TextCacheKey {
    Lyric { monitor: u32, line: u32, hash: u64 },
    TrackInfo { monitor: u32, hash: u64 },
    Weather { monitor: u32, hash: u64 },
}

pub struct PositionedBuffer {
    pub buffer: Buffer,
    pub text_key: TextCacheKey,
    pub pos: [f32; 2],
    pub color: [f32; 4],
    pub scale: f32,
    pub align: cosmic_text::Align,
}

#[derive(Clone, Copy)]
pub struct CachedGlyph {
    pub uv: [f32; 4],
    pub offset: [i32; 2],
    pub size: [u32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TextVertex {
    pub pos: [f32; 2],
    pub tex_pos: [f32; 2],
    pub color: [f32; 4],
}

pub struct TextRenderer {
    pub pipeline: wgpu::RenderPipeline,
    pub vertices: wgpu::Buffer,
    pub indices: wgpu::Buffer,
    pub num_indices: u32,
    pub bind_group: wgpu::BindGroup,
    pub texture: wgpu::Texture,
    pub vertex_capacity: usize,
    pub index_capacity: usize,
    pub glyph_cache: std::collections::HashMap<cosmic_text::CacheKey, CachedGlyph>,
    pub cache_x: u32,
    pub cache_y: u32,
    pub cache_row_height: u32,
    pub cpu_vertices: Vec<TextVertex>,
    pub cpu_indices: Vec<u32>,
}

impl TextRenderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Result<Self> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Cache Texture"),
            size: wgpu::Extent3d {
                width: GLYPH_CACHE_WIDTH,
                height: GLYPH_CACHE_HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Glyph Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Text Bind Group Layout"),
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

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Text Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(
                        &texture.create_view(&Default::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER_SRC.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Text Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x2, 2 => Float32x4],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let vertex_capacity = 2048;
        let index_capacity = 2048;

        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Vertex Buffer"),
            size: (vertex_capacity * std::mem::size_of::<TextVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let indices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Text Index Buffer"),
            size: (index_capacity * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            glyph_cache: std::collections::HashMap::new(),
            pipeline,
            vertices,
            indices,
            num_indices: 0,
            bind_group,
            texture,
            vertex_capacity,
            index_capacity,
            cache_x: 0,
            cache_y: 0,
            cache_row_height: 0,
            cpu_vertices: Vec::with_capacity(vertex_capacity),
            cpu_indices: Vec::with_capacity(index_capacity),
        })
    }
}
impl TextRenderer {
    pub fn prepare_text(
        text_renderer: &mut TextRenderer,
        queue: &wgpu::Queue,
        font_system: &mut FontSystem,
        swash_cache: &mut SwashCache,
        positioned_buffers: &mut [PositionedBuffer],
        width: f32,
        height: f32,
    ) {
        text_renderer.cpu_vertices.clear();
        text_renderer.cpu_indices.clear();

        // Optimization: Pre-calculate the constants for NDC transformation.
        // This allows us to replace multiple multiplications and divisions inside
        // the triple-nested rendering loop, saving several hundred CPU cycles per frame.
        let width_to_ndc = 2.0 / width;
        let height_to_ndc = 2.0 / height;

        for p_buf in positioned_buffers.iter_mut() {
            p_buf.buffer.shape_until_scroll(font_system, false);
        }

        for p_buf in positioned_buffers {
            let origin_x = match p_buf.align {
                cosmic_text::Align::Left => 0.0,
                cosmic_text::Align::Right => width,
                _ => width / 2.0,
            };
            let origin_y = p_buf.buffer.metrics().font_size;

            let buffer_offset_x = match p_buf.align {
                cosmic_text::Align::Left => p_buf.pos[0],
                cosmic_text::Align::Right => p_buf.pos[0] - width,
                _ => p_buf.pos[0] - width / 2.0,
            };
            let buffer_offset_y = p_buf.pos[1] - origin_y;

            for run in p_buf.buffer.layout_runs() {
                for glyph in run.glyphs.iter() {
                    // Force subpixel rendering layout to absolute 0.0 offsets. We do the real positioning in the shader!
                    let physical_glyph: cosmic_text::PhysicalGlyph =
                        glyph.physical((0.0, 0.0), 1.0);
                    let cache_key = physical_glyph.cache_key;

                    // Retrieve from cache or rasterize and pack into texture atlas
                    let mut cached = text_renderer.glyph_cache.get(&cache_key).copied();

                    if cached.is_none() {
                        if let Some(image) = swash_cache.get_image(font_system, cache_key) {
                            let img_w = image.placement.width;
                            let img_h = image.placement.height;

                            if img_w == 0 || img_h == 0 {
                                let empty_glyph = CachedGlyph {
                                    uv: [0.0, 0.0, 0.0, 0.0],
                                    offset: [0, 0],
                                    size: [0, 0],
                                };
                                text_renderer.glyph_cache.insert(cache_key, empty_glyph);
                                cached = Some(empty_glyph);
                            } else if img_w > GLYPH_CACHE_WIDTH || img_h > GLYPH_CACHE_HEIGHT {
                                tracing::warn!("Glyph ({}x{}) too large for cache!", img_w, img_h);
                            } else {
                                if text_renderer.cache_x + img_w > GLYPH_CACHE_WIDTH {
                                    text_renderer.cache_x = 0;
                                    text_renderer.cache_y += text_renderer.cache_row_height;
                                    text_renderer.cache_row_height = 0;
                                }

                                if text_renderer.cache_y + img_h > GLYPH_CACHE_HEIGHT {
                                    tracing::warn!(
                                        "Glyph cache full! Clearing and starting fresh."
                                    );
                                    text_renderer.glyph_cache.clear();
                                    text_renderer.glyph_cache.shrink_to_fit();
                                    text_renderer.cache_x = 0;
                                    text_renderer.cache_y = 0;
                                    text_renderer.cache_row_height = 0;
                                }

                                let cur_x = text_renderer.cache_x;
                                let cur_y = text_renderer.cache_y;

                                if let cosmic_text::SwashContent::Mask = image.content {
                                    queue.write_texture(
                                        wgpu::ImageCopyTexture {
                                            texture: &text_renderer.texture,
                                            mip_level: 0,
                                            origin: wgpu::Origin3d {
                                                x: cur_x,
                                                y: cur_y,
                                                z: 0,
                                            },
                                            aspect: wgpu::TextureAspect::All,
                                        },
                                        &image.data,
                                        wgpu::ImageDataLayout {
                                            offset: 0,
                                            bytes_per_row: Some(img_w),
                                            rows_per_image: Some(img_h),
                                        },
                                        wgpu::Extent3d {
                                            width: img_w,
                                            height: img_h,
                                            depth_or_array_layers: 1,
                                        },
                                    );
                                }

                                let u_min = cur_x as f32 / GLYPH_CACHE_WIDTH as f32;
                                let v_min = cur_y as f32 / GLYPH_CACHE_HEIGHT as f32;
                                let u_max = (cur_x + img_w) as f32 / GLYPH_CACHE_WIDTH as f32;
                                let v_max = (cur_y + img_h) as f32 / GLYPH_CACHE_HEIGHT as f32;

                                let new_cached = CachedGlyph {
                                    uv: [u_min, v_min, u_max, v_max],
                                    offset: [image.placement.left, image.placement.top],
                                    size: [img_w, img_h],
                                };

                                text_renderer.glyph_cache.insert(cache_key, new_cached);
                                cached = Some(new_cached);

                                text_renderer.cache_x += img_w + 1; // 1px padding
                                text_renderer.cache_row_height =
                                    text_renderer.cache_row_height.max(img_h + 1);
                            }
                        }
                    }

                    // Build vertex layout from cached/newly rasterized glyph
                    if let Some(cached) = cached {
                        if cached.size[0] == 0 || cached.size[1] == 0 {
                            continue;
                        }

                        let dx = glyph.x - origin_x;
                        let dy = run.line_y + glyph.y - origin_y;

                        let scaled_glyph_x = origin_x + dx * p_buf.scale;
                        let scaled_glyph_y = origin_y + dy * p_buf.scale;

                        let final_x = buffer_offset_x
                            + scaled_glyph_x
                            + cached.offset[0] as f32 * p_buf.scale;
                        let final_y = buffer_offset_y + scaled_glyph_y
                            - cached.offset[1] as f32 * p_buf.scale;

                        let x = final_x * width_to_ndc - 1.0;
                        let y = -(final_y * height_to_ndc - 1.0);

                        let w = (cached.size[0] as f32 * p_buf.scale) * width_to_ndc;
                        let h = (cached.size[1] as f32 * p_buf.scale) * height_to_ndc;

                        let color = p_buf.color;

                        let base_index = text_renderer.cpu_vertices.len() as u32;
                        let [u_min, v_min, u_max, v_max] = cached.uv;

                        text_renderer.cpu_vertices.extend([
                            TextVertex {
                                pos: [x, y],
                                tex_pos: [u_min, v_min],
                                color,
                            },
                            TextVertex {
                                pos: [x + w, y],
                                tex_pos: [u_max, v_min],
                                color,
                            },
                            TextVertex {
                                pos: [x, y - h],
                                tex_pos: [u_min, v_max],
                                color,
                            },
                            TextVertex {
                                pos: [x + w, y - h],
                                tex_pos: [u_max, v_max],
                                color,
                            },
                        ]);

                        text_renderer.cpu_indices.extend([
                            base_index,
                            base_index + 1,
                            base_index + 2,
                            base_index + 1,
                            base_index + 3,
                            base_index + 2,
                        ]);
                    }
                }
            }
        }
    }
}
