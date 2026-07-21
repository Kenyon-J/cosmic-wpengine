//! Custom-background (image/video/colour/gradient) texture and its blur
//! chain, plus the ambient procedural-sky pipeline that draws instead when
//! no custom background is bound. Phase 3 of the renderer decomposition
//! (`docs/PLAN-renderer-decomposition.md`) - the `ArtLayer` counterpart;
//! see its module doc for why `set_texture` is the only way to install a
//! new source texture.

use crate::modules::config::ResolvedBackground;
use crate::modules::renderer::blur::{BlurChain, KawaseBlur};

pub(crate) struct BackgroundLayer {
    texture: Option<wgpu::Texture>,
    size: Option<(u32, u32)>,
    aspect: f32,
    bind_group: Option<wgpu::BindGroup>,
    blur_chain: Option<BlurChain>,
    /// Mean colour of the custom background image, kept so text drawn over
    /// the wallpaper can pick its colour against what is actually behind it
    /// rather than the album palette.
    pub(crate) avg_color: Option<[f32; 3]>,
    /// The desktop background currently resolved and loaded, so a
    /// `ConfigUpdated` event only reloads when it actually changed.
    pub(crate) current_bg: Option<ResolvedBackground>,
    pub(crate) custom_bg_uniform_buffer: wgpu::Buffer,
    pub(crate) ambient_pipeline: wgpu::RenderPipeline,
    pub(crate) ambient_bind_group: wgpu::BindGroup,
    pub(crate) ambient_uniform_buffer: wgpu::Buffer,
}

impl BackgroundLayer {
    pub(crate) fn new(
        ambient_pipeline: wgpu::RenderPipeline,
        ambient_bind_group: wgpu::BindGroup,
        ambient_uniform_buffer: wgpu::Buffer,
        custom_bg_uniform_buffer: wgpu::Buffer,
    ) -> Self {
        Self {
            texture: None,
            size: None,
            aspect: 1.0,
            bind_group: None,
            blur_chain: None,
            avg_color: None,
            current_bg: None,
            custom_bg_uniform_buffer,
            ambient_pipeline,
            ambient_bind_group,
            ambient_uniform_buffer,
        }
    }

    pub(crate) fn texture(&self) -> Option<&wgpu::Texture> {
        self.texture.as_ref()
    }

    pub(crate) fn size(&self) -> Option<(u32, u32)> {
        self.size
    }

    pub(crate) fn aspect(&self) -> f32 {
        self.aspect
    }

    pub(crate) fn bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.bind_group.as_ref()
    }

    /// The only way to install a new source texture - see `ArtLayer::set_texture`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn set_texture(
        &mut self,
        device: &wgpu::Device,
        kawase_blur: &KawaseBlur,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        texture: wgpu::Texture,
        dimensions: (u32, u32),
        blur_enabled: bool,
    ) {
        self.aspect = (dimensions.0 as f32 / dimensions.1 as f32).max(0.001);
        self.texture = Some(texture);
        self.size = Some(dimensions);
        // Same-size recreation is even more common here than for album art:
        // solid colours are always synthesised at 16x16, gradients at
        // 1920x1080, and same-monitor wallpaper images share a resolution -
        // so without dropping the chain, changing the desktop background
        // left the frosted glass blurring the old one.
        self.blur_chain = None;
        self.rebuild(device, kawase_blur, layout, sampler, blur_enabled);
    }

    /// (Re)builds the blur chain (if enabled) and the bind group against the
    /// current texture without replacing it. A no-op while no texture/size
    /// pair is set yet.
    pub(crate) fn rebuild(
        &mut self,
        device: &wgpu::Device,
        kawase_blur: &KawaseBlur,
        layout: &wgpu::BindGroupLayout,
        sampler: &wgpu::Sampler,
        blur_enabled: bool,
    ) {
        let (Some(texture), Some(size)) = (&self.texture, self.size) else {
            return;
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        if blur_enabled {
            let up_to_date = self
                .blur_chain
                .as_ref()
                .is_some_and(|c| c.matches_source(size));
            if !up_to_date {
                self.blur_chain = Some(BlurChain::new(
                    device,
                    kawase_blur,
                    sampler,
                    &view,
                    size,
                    "Custom Background",
                ));
            }
        } else {
            self.blur_chain = None;
        }

        let blur_view = self.blur_chain.as_ref().map_or(&view, |c| c.output_view());

        self.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.custom_bg_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(blur_view),
                },
            ],
            label: Some("Custom Background Bind Group"),
        }));
    }

    /// Re-runs the blur chain over the current texture contents. A no-op
    /// while blur is disabled (no chain exists).
    pub(crate) fn run_blur(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        kawase_blur: &KawaseBlur,
        opacity: f32,
    ) {
        if let Some(chain) = &self.blur_chain {
            chain.run(device, queue, kawase_blur, opacity);
        }
    }

    /// Drops the custom background entirely (no path configured, or the
    /// image failed to load) - the ambient procedural sky takes over once
    /// `bind_group()` is `None`.
    pub(crate) fn clear(&mut self) {
        self.texture = None;
        self.size = None;
        self.bind_group = None;
        self.blur_chain = None;
        self.avg_color = None;
    }
}
