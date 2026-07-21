use super::*;

/// Uploads RGBA8 pixel data to `texture`, honouring wgpu's 256-byte row-alignment
/// requirement. `pad_buffer` is reused across calls (and only grown, never shrunk)
/// to avoid re-allocating a scratch buffer on every frame.
fn upload_rgba_to_texture(
    queue: &wgpu::Queue,
    texture: &wgpu::Texture,
    width: u32,
    height: u32,
    data: &[u8],
    pad_buffer: &mut Vec<u8>,
) {
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let unpadded_bytes_per_row = width * 4;
    let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);
    let texture_size = wgpu::Extent3d {
        width,
        height,
        depth_or_array_layers: 1,
    };

    if unpadded_bytes_per_row == padded_bytes_per_row {
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(unpadded_bytes_per_row),
                rows_per_image: Some(height),
            },
            texture_size,
        );
    } else {
        let required_size = (padded_bytes_per_row * height) as usize;
        // Skip .clear() so we don't re-zero the whole buffer every frame; resize()
        // only zero-fills newly-allocated space.
        if pad_buffer.len() < required_size {
            pad_buffer.resize(required_size, 0);
        }

        // Exact chunks + zip eliminate manual bounds checking and index arithmetic,
        // letting LLVM auto-vectorize the copy.
        for (dst_row, src_row) in pad_buffer[..required_size]
            .chunks_exact_mut(padded_bytes_per_row as usize)
            .zip(data.chunks_exact(unpadded_bytes_per_row as usize))
        {
            dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pad_buffer[..required_size],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_bytes_per_row),
                rows_per_image: Some(height),
            },
            texture_size,
        );
    }
}

impl Renderer {
    pub(crate) fn update_album_art_texture(&mut self, rgba: &image::RgbaImage) {
        // Fresh art always shows at full opacity, even if the previous art was
        // mid-fade when it arrived.
        self.art.fade = 1.0;
        let dimensions = rgba.dimensions();
        info!(
            "Creating GPU texture for album art. Dimensions: {}x{}",
            dimensions.0, dimensions.1
        );

        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Album Art Texture"),
            view_formats: &[],
        });

        upload_rgba_to_texture(
            &self.queue,
            &texture,
            dimensions.0,
            dimensions.1,
            rgba.as_raw(),
            &mut self.album_art_pad_buffer,
        );

        let blur_enabled = self.album_blur_enabled();
        self.art.set_texture(
            &self.device,
            &self.kawase_blur,
            &self.album_art_layout,
            &self.album_art_sampler,
            texture,
            dimensions,
            blur_enabled,
        );
        self.run_album_blur();
        // The art's arrival can flip the text backdrop from wallpaper to
        // album background; re-derive the text colours against it.
        self.update_text_colors();
    }

    /// True when mode 0 of album_art.wgsl will actually sample the cached
    /// blur, i.e. the frosted-glass background is in effect.
    fn blur_enabled(&self) -> bool {
        !self.state.config.appearance.disable_blur
            && self.state.config.appearance.blur_opacity >= 0.01
    }

    /// The album chain is additionally gated on the art background being
    /// shown at all, so Canvas video with a plain background doesn't pay for
    /// a re-blur on every frame.
    fn album_blur_enabled(&self) -> bool {
        self.blur_enabled() && self.state.config.appearance.album_art_background
    }

    /// (Re)creates the album art bind groups, ensuring the offscreen Kawase
    /// chain for the background blur exists first (or is dropped when blur is
    /// disabled, freeing its textures). Called whenever the source texture is
    /// recreated or the blur settings change.
    pub(crate) fn rebuild_album_bind_groups(&mut self) {
        let blur_enabled = self.album_blur_enabled();
        self.art.rebuild(
            &self.device,
            &self.kawase_blur,
            &self.album_art_layout,
            &self.album_art_sampler,
            blur_enabled,
        );
    }

    /// Re-runs the album blur chain over the current texture contents.
    /// Cheap enough for per-frame Canvas video use: the passes run at
    /// successively halved resolutions.
    pub(crate) fn run_album_blur(&self) {
        self.art.run_blur(
            &self.device,
            &self.queue,
            &self.kawase_blur,
            self.state.config.appearance.blur_opacity,
        );
    }

    pub(crate) fn run_custom_bg_blur(&self) {
        self.background.run_blur(
            &self.device,
            &self.queue,
            &self.kawase_blur,
            self.state.config.appearance.blur_opacity,
        );
    }

    /// Applies changed blur settings to the existing sources: builds or drops
    /// the offscreen chains, rebinds them, and re-runs the blur at the new
    /// strength. Textures are not re-uploaded.
    pub(crate) fn refresh_blur_chains(&mut self) {
        self.rebuild_album_bind_groups();
        self.run_album_blur();
        if self.background.bind_group().is_some() {
            self.rebuild_custom_bg_bind_group();
            self.run_custom_bg_blur();
        }
    }

    pub(crate) fn update_canvas_video_frame(&mut self, rgba: &image::RgbaImage) {
        // Fast-path: If the texture already exists and dimensions match perfectly,
        // we can copy the raw video frame bytes straight into the GPU's VRAM!
        let dimensions = rgba.dimensions();
        if self.art.size() == Some(dimensions) {
            if let Some(texture) = self.art.texture() {
                upload_rgba_to_texture(
                    &self.queue,
                    texture,
                    dimensions.0,
                    dimensions.1,
                    rgba.as_raw(),
                    &mut self.video_frame_buffer,
                );
            }
            self.run_album_blur();
            return;
        }

        // Slow-path: If dimensions changed (e.g. switching from square album art to 9:16 Canvas video),
        // this will rebuild the wgpu texture and elegantly crossfade into the video loop!
        self.update_album_art_texture(rgba);
    }

    pub(crate) fn update_background_video_frame(&mut self, rgba: &image::RgbaImage) {
        let dimensions = rgba.dimensions();
        if self.background.size() == Some(dimensions) {
            if let Some(texture) = self.background.texture() {
                upload_rgba_to_texture(
                    &self.queue,
                    texture,
                    dimensions.0,
                    dimensions.1,
                    rgba.as_raw(),
                    &mut self.video_frame_buffer,
                );
            }
            self.run_custom_bg_blur();
            return;
        }
        self.load_custom_background_from_image(rgba);
    }

    pub(crate) fn update_theme_colors(&mut self) {
        let get_vis_colors =
            |palette: Option<&[[f32; 3]]>, theme: &ThemeLayout| -> ([f32; 3], [f32; 3]) {
                let top = theme.visualiser.color_top;
                let bottom = theme.visualiser.color_bottom;

                if let (Some(top_val), Some(bottom_val)) = (top, bottom) {
                    (top_val, bottom_val)
                } else {
                    match palette {
                        Some(p) if p.len() >= 2 => (top.unwrap_or(p[0]), bottom.unwrap_or(p[1])),
                        Some(p) if p.len() == 1 => (
                            top.unwrap_or(p[0]),
                            bottom.unwrap_or([p[0][0] * 0.5, p[0][1] * 0.5, p[0][2] * 0.5]),
                        ),
                        _ => (
                            top.unwrap_or([1.0, 0.2, 0.5]),
                            bottom.unwrap_or([0.2, 0.5, 1.0]),
                        ),
                    }
                }
            };

        let get_art_color = |palette: Option<&[[f32; 3]]>| -> [f32; 3] {
            palette
                .and_then(|p| p.first())
                .copied()
                .unwrap_or([0.1, 0.1, 0.1])
        };

        // Update Visualizer colors
        self.vis_prev_colors = get_vis_colors(self.state.previous_palette.as_deref(), &self.theme);
        self.vis_target_colors = get_vis_colors(
            self.state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref()),
            &self.theme,
        );

        // Update Album Art colors
        self.art.prev_color = get_art_color(self.state.previous_palette.as_deref());
        self.art.target_color = get_art_color(
            self.state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref()),
        );
    }

    /// Loads whatever background the COSMIC desktop is configured with:
    /// images come off disk, while colour/gradient wallpapers have no file
    /// (cosmic-bg paints them directly) so we synthesise a matching texture.
    pub fn load_resolved_background(&mut self, bg: Option<&ResolvedBackground>) {
        match bg {
            None => self.load_custom_background(None),
            Some(ResolvedBackground::Image(path)) => self.load_custom_background(Some(path)),
            Some(ResolvedBackground::Colour(colour)) => {
                info!("Loading solid-colour desktop background");
                let img = super::super::utils::solid_colour_image(*colour);
                self.load_custom_background_from_image(&img);
            }
            Some(ResolvedBackground::Gradient { colors, angle_deg }) => {
                info!(
                    "Loading gradient desktop background ({} stops)",
                    colors.len()
                );
                let img = super::super::utils::gradient_image(colors, *angle_deg, 1920, 1080);
                self.load_custom_background_from_image(&img);
            }
        }
    }

    pub fn load_custom_background(&mut self, path: Option<&str>) {
        let Some(path) = path else {
            self.background.clear();
            self.update_text_colors();
            return;
        };

        info!("Loading custom background from {}", path);
        let img = match image::open(path) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                warn!("Failed to load custom background: {}", e);
                self.background.clear();
                self.update_text_colors();
                return;
            }
        };

        self.load_custom_background_from_image(&img);
    }

    pub fn load_custom_background_from_image(&mut self, img: &image::RgbaImage) {
        let dimensions = img.dimensions();
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            size: texture_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Custom Background Texture"),
            view_formats: &[],
        });

        upload_rgba_to_texture(
            &self.queue,
            &texture,
            dimensions.0,
            dimensions.1,
            img.as_raw(),
            &mut self.album_art_pad_buffer,
        );

        self.background.avg_color = Some(crate::modules::colour::average_colour(img));
        let blur_enabled = self.blur_enabled();
        self.background.set_texture(
            &self.device,
            &self.kawase_blur,
            &self.album_art_layout,
            &self.album_art_sampler,
            texture,
            dimensions,
            blur_enabled,
        );
        self.run_custom_bg_blur();
        self.update_text_colors();
    }

    /// Custom-background counterpart of [`Self::rebuild_album_bind_groups`].
    pub(crate) fn rebuild_custom_bg_bind_group(&mut self) {
        let blur_enabled = self.blur_enabled();
        self.background.rebuild(
            &self.device,
            &self.kawase_blur,
            &self.album_art_layout,
            &self.album_art_sampler,
            blur_enabled,
        );
    }

    pub(crate) fn update_text_colors(&mut self) {
        // A user-picked colour overrides the adaptive logic entirely.
        if let Some(c) = self.state.config.appearance.text_color {
            self.primary_text_color = [c[0], c[1], c[2], 1.0];
            self.secondary_text_color = [c[0], c[1], c[2], 0.7];
            self.text_color_diff = [0.0, 0.0, 0.0, 0.3];
            return;
        }

        let palette = self
            .state
            .current_track
            .as_ref()
            .and_then(|t| t.palette.as_deref());

        // Mirror draw.rs's backdrop selection: the album-art / colour
        // backgrounds only show when art exists and the flag is set;
        // otherwise the text sits on the desktop wallpaper, so its colour
        // must contrast with that instead of the album palette.
        let appearance = &self.state.config.appearance;
        let has_art = self.art.fg_bind_group().is_some()
            || self.state.config.mode == crate::modules::config::WallpaperMode::AlbumArt;
        let color_bg_shown = has_art && appearance.album_color_background;
        let album_backdrop =
            has_art && (appearance.album_art_background || appearance.album_color_background);

        let mut text_bg_color = if album_backdrop {
            palette
                .and_then(|p| p.first())
                .copied()
                .unwrap_or([0.1, 0.1, 0.1])
        } else {
            self.background.avg_color.unwrap_or([0.1, 0.1, 0.1])
        };

        // The frosted-glass pass composites its neutral tint over the
        // backdrop (album_art.wgsl GLASS_TINT, #1B1B1B), so judge contrast
        // against the dimmed result. Colour backgrounds are never frosted.
        if !appearance.disable_blur && !color_bg_shown {
            text_bg_color = crate::modules::colour::lerp_colour(
                text_bg_color,
                [0.106, 0.106, 0.106],
                appearance.blur_opacity * 0.45,
            );
        }

        let text_accent = palette
            .and_then(|p| p.get(1).or_else(|| p.first()))
            .copied()
            .unwrap_or([1.0, 1.0, 1.0]);

        let luminance = crate::modules::colour::relative_luminance(text_bg_color);
        if luminance > 0.179 {
            // Dark text for bright backgrounds, tinted with the accent color
            let tint = [
                text_accent[0] * 0.3,
                text_accent[1] * 0.3,
                text_accent[2] * 0.3,
            ];
            let rgb = crate::modules::colour::ensure_contrast(tint, text_bg_color, 4.5);
            self.primary_text_color = [rgb[0], rgb[1], rgb[2], 1.0];
            self.secondary_text_color = [rgb[0], rgb[1], rgb[2], 0.7];
        } else {
            // Light text for dark backgrounds, lightly tinted with the accent color
            let tint = [
                text_accent[0] * 0.3 + 0.7,
                text_accent[1] * 0.3 + 0.7,
                text_accent[2] * 0.3 + 0.7,
            ];
            let rgb = crate::modules::colour::ensure_contrast(tint, text_bg_color, 4.5);
            self.primary_text_color = [rgb[0], rgb[1], rgb[2], 1.0];
            self.secondary_text_color = [rgb[0], rgb[1], rgb[2], 0.45];
        }

        self.text_color_diff = [
            self.primary_text_color[0] - self.secondary_text_color[0],
            self.primary_text_color[1] - self.secondary_text_color[1],
            self.primary_text_color[2] - self.secondary_text_color[2],
            self.primary_text_color[3] - self.secondary_text_color[3],
        ];
    }

    pub(crate) fn update_weather_state(&mut self) {
        self.is_weather_active = self.state.config.weather.enabled
            && !self.state.config.weather.hide_effects
            && self.state.weather.as_ref().is_some_and(|w| {
                matches!(
                    w.condition,
                    WeatherCondition::Rain
                        | WeatherCondition::Snow
                        | WeatherCondition::Thunderstorm
                )
            });

        if self.is_weather_active {
            if let Some(weather) = &self.state.weather {
                self.active_particles = match weather.condition {
                    WeatherCondition::Rain => 800,
                    WeatherCondition::Thunderstorm => 1500,
                    WeatherCondition::Snow => 2500,
                    _ => 0,
                };
                match weather.condition {
                    WeatherCondition::Rain | WeatherCondition::Thunderstorm => {
                        self.weather_gravity = 0.85;
                        self.weather_wind_x = 0.15;
                        self.weather_type = 2;
                    }
                    WeatherCondition::Snow => {
                        self.weather_gravity = 0.2;
                        self.weather_wind_x = 0.5;
                        self.weather_type = 3;
                    }
                    WeatherCondition::Cloudy | WeatherCondition::Fog => {
                        self.weather_gravity = 0.5;
                        self.weather_wind_x = 0.1;
                        self.weather_type = 1;
                    }
                    _ => {
                        self.weather_gravity = 0.5;
                        self.weather_wind_x = 0.1;
                        self.weather_type = 0;
                    }
                }
            } else {
                self.active_particles = 0;
                self.is_weather_active = false;
            }
        } else {
            self.active_particles = 0;
            // Also update weather_type for the sky gradient even if effects are hidden
            if let Some(weather) = &self.state.weather {
                self.weather_type = match weather.condition {
                    WeatherCondition::Clear | WeatherCondition::PartlyCloudy => 0,
                    WeatherCondition::Cloudy | WeatherCondition::Fog => 1,
                    WeatherCondition::Rain | WeatherCondition::Thunderstorm => 2,
                    WeatherCondition::Snow => 3,
                };
            } else {
                self.weather_type = 0;
            }
        }
    }

    pub(crate) fn update_weather_string(&mut self) {
        use crate::modules::renderer::utils::hash_str;
        if let Some(weather) = &self.state.weather {
            let mut val = weather.temperature_celsius;
            let mut unit = "C";
            if self.state.config.weather.temperature_unit == TemperatureUnit::Fahrenheit {
                val = (val * 9.0 / 5.0) + 32.0;
                unit = "F";
            }
            let condition_str = match weather.condition {
                WeatherCondition::Clear => "Clear",
                WeatherCondition::PartlyCloudy => "Partly Cloudy",
                WeatherCondition::Cloudy => "Cloudy",
                WeatherCondition::Rain => "Rain",
                WeatherCondition::Snow => "Snow",
                WeatherCondition::Thunderstorm => "Thunderstorm",
                WeatherCondition::Fog => "Fog",
            };
            self.cached_weather_str = format!("{} {:.1}°{}", condition_str, val, unit);
            self.cached_weather_hash = hash_str(&self.cached_weather_str);
        } else {
            self.cached_weather_str.clear();
            self.cached_weather_hash = 0;
        }
    }
}
