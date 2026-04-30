use super::*;
impl Renderer {
    pub(crate) fn update_album_art_texture(&mut self, rgba: &image::RgbaImage) {
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

        // Guarantee dimensions are compatible with wgpu's 256-byte row alignment!
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded_bytes_per_row = dimensions.0 * 4;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

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

        if unpadded_bytes_per_row == padded_bytes_per_row {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                rgba.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(unpadded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        } else {
            let required_size = (padded_bytes_per_row * dimensions.1) as usize;
            // Optimization: Re-use the existing buffer if possible. resize(..., 0) only zero-fills
            // newly-allocated space, so by skipping .clear() at the end of the previous frame,
            // we avoid zeroing the entire buffer every single frame.
            if self.album_art_pad_buffer.len() < required_size {
                self.album_art_pad_buffer.resize(required_size, 0);
            }

            // Optimization: Use exact chunks and zip to eliminate manual bounds checking
            // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
            for (dst_row, src_row) in self.album_art_pad_buffer[..required_size]
                .chunks_exact_mut(padded_bytes_per_row as usize)
                .zip(rgba.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
            {
                dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
            }
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.album_art_pad_buffer[..required_size],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let bg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.album_art_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.album_art_bg_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.album_art_sampler),
                },
            ],
            label: Some("Album Art BG Bind Group"),
        });

        let fg_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.album_art_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.album_art_fg_uniform_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.album_art_sampler),
                },
            ],
            label: Some("Album Art FG Bind Group"),
        });

        self.album_art_bg_bind_group = Some(bg_bind_group);
        self.album_art_fg_bind_group = Some(fg_bind_group);
        self.current_album_texture = Some(texture);
    }

    pub(crate) fn update_canvas_video_frame(&mut self, rgba: &image::RgbaImage) {
        // Fast-path: If the texture already exists and dimensions match perfectly,
        // we can copy the raw video frame bytes straight into the GPU's VRAM!
        if let Some(texture) = &self.current_album_texture {
            let dimensions = rgba.dimensions();
            if texture.size().width == dimensions.0 && texture.size().height == dimensions.1 {
                let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                let unpadded_bytes_per_row = dimensions.0 * 4;
                let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

                if unpadded_bytes_per_row == padded_bytes_per_row {
                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        rgba.as_raw(),
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(unpadded_bytes_per_row),
                            rows_per_image: Some(dimensions.1),
                        },
                        texture.size(),
                    );
                } else {
                    let required_size = (padded_bytes_per_row * dimensions.1) as usize;
                    // Optimization: Skip .clear() to avoid redundant zero-filling by .resize()
                    if self.video_frame_buffer.len() < required_size {
                        self.video_frame_buffer.resize(required_size, 0);
                    }

                    // Optimization: Use exact chunks and zip to eliminate manual bounds checking
                    // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
                    for (dst_row, src_row) in self.video_frame_buffer[..required_size]
                        .chunks_exact_mut(padded_bytes_per_row as usize)
                        .zip(rgba.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
                    {
                        dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
                    }

                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &self.video_frame_buffer[..required_size],
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_bytes_per_row),
                            rows_per_image: Some(dimensions.1),
                        },
                        texture.size(),
                    );
                }
                return;
            }
        }

        // Slow-path: If dimensions changed (e.g. switching from square album art to 9:16 Canvas video),
        // this will rebuild the wgpu texture and elegantly crossfade into the video loop!
        self.update_album_art_texture(rgba);
    }

    pub(crate) fn update_background_video_frame(&mut self, rgba: &image::RgbaImage) {
        if let Some(texture) = &self.current_custom_bg_texture {
            let dimensions = rgba.dimensions();
            if texture.size().width == dimensions.0 && texture.size().height == dimensions.1 {
                let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
                let unpadded_bytes_per_row = dimensions.0 * 4;
                let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

                if unpadded_bytes_per_row == padded_bytes_per_row {
                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        rgba.as_raw(),
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(unpadded_bytes_per_row),
                            rows_per_image: Some(dimensions.1),
                        },
                        texture.size(),
                    );
                } else {
                    let required_size = (padded_bytes_per_row * dimensions.1) as usize;
                    // Optimization: Skip .clear() to avoid redundant zero-filling by .resize()
                    if self.video_frame_buffer.len() < required_size {
                        self.video_frame_buffer.resize(required_size, 0);
                    }

                    // Optimization: Use exact chunks and zip to eliminate manual bounds checking
                    // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
                    for (dst_row, src_row) in self.video_frame_buffer[..required_size]
                        .chunks_exact_mut(padded_bytes_per_row as usize)
                        .zip(rgba.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
                    {
                        dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
                    }

                    self.queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &self.video_frame_buffer[..required_size],
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(padded_bytes_per_row),
                            rows_per_image: Some(dimensions.1),
                        },
                        texture.size(),
                    );
                }
                return;
            }
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
        self.art_prev_color = get_art_color(self.state.previous_palette.as_deref());
        self.art_target_color = get_art_color(
            self.state
                .current_track
                .as_ref()
                .and_then(|t| t.palette.as_deref()),
        );
    }

    pub fn load_custom_background(&mut self, path: Option<&str>) {
        let Some(path) = path else {
            self.custom_bg_bind_group = None;
            self.current_custom_bg_texture = None;
            return;
        };

        info!("Loading custom background from {}", path);
        let img = match image::open(path) {
            Ok(i) => i.to_rgba8(),
            Err(e) => {
                warn!("Failed to load custom background: {}", e);
                self.custom_bg_bind_group = None;
                self.current_custom_bg_texture = None;
                return;
            }
        };

        self.load_custom_background_from_image(&img);
    }

    pub fn load_custom_background_from_image(&mut self, img: &image::RgbaImage) {
        let dimensions = img.dimensions();
        self.current_custom_bg_size = Some(dimensions);
        let texture_size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        // Guarantee dimensions are compatible with wgpu's 256-byte row alignment!
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let unpadded_bytes_per_row = dimensions.0 * 4;
        let padded_bytes_per_row = (unpadded_bytes_per_row + align - 1) & !(align - 1);

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

        if unpadded_bytes_per_row == padded_bytes_per_row {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                img.as_raw(),
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(unpadded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        } else {
            let required_size = (padded_bytes_per_row * dimensions.1) as usize;
            // Optimization: Avoid redundant zero-filling by reuse of the pad buffer
            if self.album_art_pad_buffer.len() < required_size {
                self.album_art_pad_buffer.resize(required_size, 0);
            }

            // Optimization: Use exact chunks and zip to eliminate manual bounds checking
            // and index arithmetic, allowing LLVM to auto-vectorize the memory copy.
            for (dst_row, src_row) in self.album_art_pad_buffer[..required_size]
                .chunks_exact_mut(padded_bytes_per_row as usize)
                .zip(img.as_raw().chunks_exact(unpadded_bytes_per_row as usize))
            {
                dst_row[..unpadded_bytes_per_row as usize].copy_from_slice(src_row);
            }
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &self.album_art_pad_buffer[..required_size],
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(dimensions.1),
                },
                texture_size,
            );
        }

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.custom_bg_bind_group =
            Some(self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.album_art_layout,
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
                        resource: wgpu::BindingResource::Sampler(&self.album_art_sampler),
                    },
                ],
                label: Some("Custom Background Bind Group"),
            }));
        self.current_custom_bg_texture = Some(texture);
    }

    pub(crate) fn update_text_colors(&mut self) {
        let palette = self
            .state
            .current_track
            .as_ref()
            .and_then(|t| t.palette.as_deref());

        let text_bg_color = palette
            .and_then(|p| p.first())
            .copied()
            .unwrap_or([0.1, 0.1, 0.1]);
        let text_accent = palette
            .and_then(|p| p.get(1).or_else(|| p.first()))
            .copied()
            .unwrap_or([1.0, 1.0, 1.0]);

        let luminance =
            0.299 * text_bg_color[0] + 0.587 * text_bg_color[1] + 0.114 * text_bg_color[2];
        if luminance > 0.55 {
            // Dark text for bright backgrounds, tinted with the accent color
            let tint = [
                text_accent[0] * 0.3,
                text_accent[1] * 0.3,
                text_accent[2] * 0.3,
            ];
            self.primary_text_color = [tint[0], tint[1], tint[2], 1.0];
            self.secondary_text_color = [tint[0], tint[1], tint[2], 0.7];
        } else {
            // Light text for dark backgrounds, lightly tinted with the accent color
            let tint = [
                text_accent[0] * 0.3 + 0.7,
                text_accent[1] * 0.3 + 0.7,
                text_accent[2] * 0.3 + 0.7,
            ];
            self.primary_text_color = [tint[0], tint[1], tint[2], 1.0];
            self.secondary_text_color = [tint[0], tint[1], tint[2], 0.45];
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
