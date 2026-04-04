use super::types::Particle;

pub(crate) fn create_album_art_pipeline(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    config_format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    wgpu::BindGroupLayout,
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::Texture,
    wgpu::Sampler,
) {
    // --- Album Art Pipeline Setup ---
    let empty_texture = device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("Empty Album Art Texture"),
        view_formats: &[],
    });

    queue.write_texture(
        wgpu::ImageCopyTexture {
            texture: &empty_texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[0, 0, 0, 255],
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );

    let album_art_bg_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Album Art BG Uniform Buffer"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let album_art_fg_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Album Art FG Uniform Buffer"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let album_art_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("Album Art Bind Group Layout"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(64),
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

    let album_art_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Album Art Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../album_art.wgsl").into()),
    });

    let album_art_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Album Art Pipeline Layout"),
            bind_group_layouts: &[&album_art_layout],
            push_constant_ranges: &[],
        });

    let album_art_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Album Art Render Pipeline"),
        layout: Some(&album_art_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &album_art_shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &album_art_shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: config_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        multiview: None,
    });

    let album_art_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    (
        album_art_pipeline,
        album_art_layout,
        album_art_bg_uniform_buffer,
        album_art_fg_uniform_buffer,
        empty_texture,
        album_art_sampler,
    )
}

pub(crate) fn create_ambient_pipeline(
    device: &wgpu::Device,
    config_format: wgpu::TextureFormat,
) -> (
    wgpu::RenderPipeline,
    wgpu::BindGroup,
    wgpu::Buffer,
    wgpu::Buffer,
) {
    // --- Ambient Pipeline Setup ---
    let ambient_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Ambient Uniform Buffer"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let ambient_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Ambient Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(64),
                },
                count: None,
            }],
        });

    let ambient_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Ambient Bind Group"),
        layout: &ambient_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: ambient_uniform_buffer.as_entire_binding(),
        }],
    });

    let ambient_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Ambient Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../ambient.wgsl").into()),
    });

    let ambient_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Ambient Pipeline Layout"),
        bind_group_layouts: &[&ambient_bind_group_layout],
        push_constant_ranges: &[],
    });

    let ambient_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Ambient Render Pipeline"),
        layout: Some(&ambient_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &ambient_shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &ambient_shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: config_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
        multiview: None,
    });

    let custom_bg_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Custom BG Uniform Buffer"),
        size: 64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    (
        ambient_pipeline,
        ambient_bind_group,
        ambient_uniform_buffer,
        custom_bg_uniform_buffer,
    )
}

pub(crate) fn create_weather_pipelines(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    config_format: wgpu::TextureFormat,
) -> (
    wgpu::Buffer,
    wgpu::Buffer,
    wgpu::BindGroup,
    wgpu::ComputePipeline,
    wgpu::BindGroup,
    wgpu::RenderPipeline,
) {
    // --- Weather Compute Pipeline Setup ---
    let max_particles = 2500;
    let mut initial_particles = Vec::with_capacity(max_particles);
    for i in 0..max_particles {
        initial_particles.push(Particle {
            pos: [
                (i as f32 * 12.9898).sin().fract() * 2.0 - 0.5, // Random X scatter
                (i as f32 * 7.2345).sin().fract() * 2.4 - 1.2, // Naturally spread across the entire vertical space
            ],
            vel: [0.0, 0.5 + (i as f32 % 5.0) * 0.1], // Base downward velocity
            lifetime: 5.0 + (i as f32 % 5.0),
            scale: 1.0,
        });
    }

    let particle_buffer_size =
        (initial_particles.len() * std::mem::size_of::<Particle>()) as wgpu::BufferAddress;
    let particle_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Particle Storage Buffer"),
        size: particle_buffer_size,
        usage: wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::VERTEX
            | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let particles_bytes = unsafe {
        std::slice::from_raw_parts(
            initial_particles.as_ptr() as *const u8,
            particle_buffer_size as usize,
        )
    };
    queue.write_buffer(&particle_buffer, 0, particles_bytes);

    let weather_compute_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("Weather Compute Uniform Buffer"),
        size: 16, // delta_time, wind_x, gravity, padding
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let weather_compute_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Weather Compute Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    // Storage Buffer (Read/Write)
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(particle_buffer_size),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    // Uniform Buffer (delta_time, physics)
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(16),
                    },
                    count: None,
                },
            ],
        });

    let weather_compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Weather Compute Bind Group"),
        layout: &weather_compute_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: particle_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: weather_compute_uniform_buffer.as_entire_binding(),
            },
        ],
    });

    let weather_compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Weather Compute Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../weather_compute.wgsl").into()),
    });

    let weather_compute_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Weather Compute Pipeline Layout"),
            bind_group_layouts: &[&weather_compute_bind_group_layout],
            push_constant_ranges: &[],
        });

    let weather_compute_pipeline =
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Weather Compute Pipeline"),
            layout: Some(&weather_compute_pipeline_layout),
            module: &weather_compute_shader,
            entry_point: "main",
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        });

    // --- Weather Render Pipeline Setup ---
    let weather_render_bind_group_layout =
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Weather Render Bind Group Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(particle_buffer_size),
                },
                count: None,
            }],
        });

    let weather_render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("Weather Render Bind Group"),
        layout: &weather_render_bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: particle_buffer.as_entire_binding(),
        }],
    });

    let weather_render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("Weather Render Shader"),
        source: wgpu::ShaderSource::Wgsl(include_str!("../weather_render.wgsl").into()),
    });

    let weather_render_pipeline_layout =
        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Weather Render Pipeline Layout"),
            bind_group_layouts: &[&weather_render_bind_group_layout],
            push_constant_ranges: &[],
        });

    let weather_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("Weather Render Pipeline"),
        layout: Some(&weather_render_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &weather_render_shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &weather_render_shader,
            entry_point: "fs_main",
            targets: &[Some(wgpu::ColorTargetState {
                format: config_format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
    });

    (
        particle_buffer,
        weather_compute_uniform_buffer,
        weather_compute_bind_group,
        weather_compute_pipeline,
        weather_render_bind_group,
        weather_render_pipeline,
    )
}
