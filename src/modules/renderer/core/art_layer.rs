//! Album-art texture, its blur chain, and fade/colour transition state.
//! Phase 3 of the renderer decomposition (`docs/PLAN-renderer-decomposition.md`).
//!
//! `set_texture` is the only way to install a new source texture: it drops
//! the old blur chain and rebuilds it plus both bind groups atomically, so
//! the 2026-07-19 stale-blur-chain bug (a same-size texture swap left the
//! chain bound to the previous texture's view) can't recur structurally -
//! there is no path that updates the texture without also rebuilding what
//! depends on it.

use crate::modules::renderer::blur::{BlurChain, KawaseBlur};
use std::time::Instant;

pub(crate) struct ArtLayer {
    texture: Option<wgpu::Texture>,
    size: Option<(u32, u32)>,
    aspect: f32,
    bg_bind_group: Option<wgpu::BindGroup>,
    fg_bind_group: Option<wgpu::BindGroup>,
    blur_chain: Option<BlurChain>,
    pub(crate) bg_uniform_buffer: wgpu::Buffer,
    pub(crate) fg_uniform_buffer: wgpu::Buffer,
    /// Opacity multiplier for the album art (bg + fg). Normally 1.0; eases
    /// to 0.0 once `pending_deadline` expires, after which `clear()` drops
    /// the art.
    pub(crate) fade: f32,
    /// While a new track's art is still being fetched in the background,
    /// the previous track's art/palette stay on screen. If nothing has
    /// arrived by this deadline, they fade out rather than lingering stale
    /// forever.
    pub(crate) pending_deadline: Option<Instant>,
    pub(crate) target_color: [f32; 3],
    pub(crate) prev_color: [f32; 3],
}

impl ArtLayer {
    pub(crate) fn new(
        empty_texture: wgpu::Texture,
        bg_uniform_buffer: wgpu::Buffer,
        fg_uniform_buffer: wgpu::Buffer,
    ) -> Self {
        Self {
            // Seeds current_album_texture at init so the bg/fg bind groups
            // always have a valid (if blank) texture to point at; size
            // deliberately stays None - rebuild()/set_texture() gate on the
            // pair together, so no bind group is built until real art
            // arrives. Preserve this asymmetry rather than "fixing" it.
            texture: Some(empty_texture),
            size: None,
            aspect: 1.0,
            bg_bind_group: None,
            fg_bind_group: None,
            blur_chain: None,
            bg_uniform_buffer,
            fg_uniform_buffer,
            fade: 1.0,
            pending_deadline: None,
            target_color: [0.1, 0.1, 0.1],
            prev_color: [0.1, 0.1, 0.1],
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

    pub(crate) fn bg_bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.bg_bind_group.as_ref()
    }

    pub(crate) fn fg_bind_group(&self) -> Option<&wgpu::BindGroup> {
        self.fg_bind_group.as_ref()
    }

    /// The only way to install a new source texture - see the module doc.
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
        // The blur chain binds the source texture's view at build time, so
        // the texture just recreated above invalidates it even when the
        // dimensions match - and matching is the norm, not the exception
        // (streaming services serve fixed-size art, e.g. Spotify's 640x640).
        self.blur_chain = None;
        self.rebuild(device, kawase_blur, layout, sampler, blur_enabled);
    }

    /// (Re)builds the blur chain (if enabled) and both bind groups against
    /// the current texture without replacing it - used when blur settings
    /// change but the source art hasn't. A no-op while no texture/size pair
    /// is set yet (the `empty_texture` / `None` size asymmetry from `new`).
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
                    "Album Art",
                ));
            }
        } else {
            self.blur_chain = None;
        }

        // With blur disabled, mode 0 short-circuits before sampling
        // blurred_art; the sharp view just keeps the binding valid.
        let blur_view = self.blur_chain.as_ref().map_or(&view, |c| c.output_view());

        self.bg_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.bg_uniform_buffer.as_entire_binding(),
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
            label: Some("Album Art BG Bind Group"),
        }));

        self.fg_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.fg_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                // The foreground never samples blurred_art.
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
            ],
            label: Some("Album Art FG Bind Group"),
        }));
    }

    /// Re-runs the blur chain over the current texture contents. Cheap
    /// enough for per-frame Canvas video use: the passes run at
    /// successively halved resolutions. A no-op while blur is disabled (no
    /// chain exists).
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

    /// Drops the art entirely: the fetch grace period expired with nothing
    /// new arriving, or the player shut down. Does not touch `fade` -
    /// callers that need it reset (the grace-period path) do so themselves,
    /// matching the pre-decomposition behaviour where shutdown left it
    /// untouched.
    pub(crate) fn clear(&mut self) {
        self.texture = None;
        self.size = None;
        self.bg_bind_group = None;
        self.fg_bind_group = None;
        self.blur_chain = None;
        self.pending_deadline = None;
    }
}
