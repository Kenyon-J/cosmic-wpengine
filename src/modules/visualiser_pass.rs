use anyhow::Result;
use wgpu::util::DeviceExt;

pub struct VisualiserPass {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group: wgpu::BindGroup,
    pub uniform_buffer: wgpu::Buffer,
    pub bands_buffer: wgpu::Buffer,
    layout: wgpu::BindGroupLayout,
    shader_src: String,
}

impl VisualiserPass {
    pub async fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        band_count: usize,
        style: &str,
    ) -> Result<Self> {
        let mut uniform_data = Vec::with_capacity(96);
        // Placeholder init data; the renderer loop immediately overwrites this
        for _ in 0..24 {
            uniform_data.extend_from_slice(&[0u8; 4]);
        }

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Visualiser Uniform Buffer"),
            contents: &uniform_data,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bands_size = (band_count * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let bands_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Audio Bands Buffer"),
            size: bands_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Visualiser Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(96),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(4), // Minimum required size
                    },
                    count: None,
                },
            ],
        });

        let bind_group = Self::create_bind_group(device, &layout, &uniform_buffer, &bands_buffer);

        let theme = super::config::ThemeLayout::load(style);
        let mut shader_src = Self::load_shader_source(theme.visualiser.shader.as_deref());

        let mut pipeline = Self::create_pipeline(device, format, &layout, &shader_src).await;

        if pipeline.is_none() {
            tracing::warn!("Falling back to 'bars' due to invalid initial shader.");
            shader_src = include_str!("visualiser.wgsl").to_string();
            pipeline = Self::create_pipeline(device, format, &layout, &shader_src).await;
        }

        Ok(Self {
            pipeline: pipeline
                .ok_or_else(|| anyhow::anyhow!("Failed to create visualiser pipeline"))?,
            bind_group,
            uniform_buffer,
            bands_buffer,
            layout,
            shader_src,
        })
    }

    fn create_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
        bands_buf: &wgpu::Buffer,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Visualiser Bind Group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: bands_buf.as_entire_binding(),
                },
            ],
        })
    }

    pub async fn reload(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        style: &str,
        band_count: usize,
    ) {
        let bands_size = (band_count * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        if self.bands_buffer.size() != bands_size {
            self.bands_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Audio Bands Buffer"),
                size: bands_size,
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.bind_group = Self::create_bind_group(
                device,
                &self.layout,
                &self.uniform_buffer,
                &self.bands_buffer,
            );
        }

        let theme = super::config::ThemeLayout::load(style);
        let new_shader_src = Self::load_shader_source(theme.visualiser.shader.as_deref());

        // WGSL pipeline compilation is extremely expensive. Only rebuild if the shader code actually changed!
        if self.shader_src != new_shader_src {
            if let Some(new_pipeline) =
                Self::create_pipeline(device, format, &self.layout, &new_shader_src).await
            {
                self.pipeline = new_pipeline;
                self.shader_src = new_shader_src;
            } else {
                tracing::warn!("Keeping previous visualiser shader due to compilation failure.");
            }
        }
    }

    fn load_shader_source(custom_shader: Option<&str>) -> String {
        if let Some(shader_name) = custom_shader {
            let safe_shader_name = std::path::Path::new(shader_name)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("visualiser.wgsl");
            let path = super::config::Config::config_dir()
                .join("shaders")
                .join(safe_shader_name);
            std::fs::read_to_string(&path).unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to read custom shader '{safe_shader_name}': {e}. Falling back to default."
                );
                include_str!("visualiser.wgsl").to_string()
            })
        } else {
            include_str!("visualiser.wgsl").to_string()
        }
    }

    async fn create_pipeline(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        layout: &wgpu::BindGroupLayout,
        shader_src: &str,
    ) -> Option<wgpu::RenderPipeline> {
        device.push_error_scope(wgpu::ErrorFilter::Validation);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Visualiser Shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Visualiser Pipeline Layout"),
            bind_group_layouts: &[layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Visualiser Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[],
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
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        if let Some(err) = device.pop_error_scope().await {
            tracing::error!("Shader validation error:\n{}", err);
            None
        } else {
            Some(pipeline)
        }
    }
}
