use super::*;
impl Renderer {
    pub(crate) async fn handle_event(&mut self, event: Event) {
        use crate::modules::renderer::utils::hash_str;
        match event {
            Event::ConfigUpdated(config, theme_layout) => {
                let _ = self.show_lyrics_tx.send(config.audio.show_lyrics);

                let new_bg = config.appearance.resolved_background_path().await;
                if new_bg != self.current_bg_path {
                    self.load_custom_background(new_bg.as_deref());
                    self.current_bg_path = new_bg.clone();
                }

                if config.audio.bands != self.state.config.audio.bands {
                    self.state.audio_bands = vec![0.0; config.audio.bands].into_boxed_slice();
                    self.state.audio_waveform = vec![0.0; config.audio.bands].into_boxed_slice();
                    self.state.audio_energy = 0.0;
                    self.a_weighting_curve =
                        crate::modules::renderer::utils::build_a_weighting_curve(
                            config.audio.bands,
                        );
                    self.frequency_bin_ranges =
                        crate::modules::renderer::utils::build_frequency_bin_ranges(
                            config.audio.bands,
                        );
                    self.waveform_bin_ranges =
                        crate::modules::renderer::utils::build_waveform_bin_ranges(
                            config.audio.bands,
                        );
                }

                // Always reload the shader pipeline so live WGSL edits apply instantly!
                let format = self.outputs[0].config.format;
                self.visualiser_pass
                    .reload(
                        &self.device,
                        format,
                        &config.audio.style,
                        config.audio.bands,
                    )
                    .await;

                // Always reload the theme layout so live edits to the .toml apply instantly!
                self.theme = *theme_layout;
                self.state.config = *config;
                self.update_theme_colors();

                // Optimization: Clear and shrink the text buffer cache on config updates to ensure
                // changes like font family or size are applied immediately and memory is reclaimed.
                self.text_buffer_cache.clear();
                self.text_buffer_cache.shrink_to_fit();

                self.is_waveform_style = self.state.config.audio.style == "waveform";
                self.update_weather_string();
                info!("Live settings applied!");
            }
            Event::TrackChanged(mut track) => {
                self.text_buffer_cache.clear(); // Free old shaped lyrics from memory!
                self.text_buffer_cache.shrink_to_fit();

                // Optimization: Don't shrink staging buffers to fit on track changes;
                // keep the allocations ready for the next track's album art or video loops.
                // Recreate SwashCache to flush its internal rasterized glyph memory
                self.swash_cache = SwashCache::new();
                self.text_renderer.glyph_cache.clear();
                self.text_renderer.glyph_cache.shrink_to_fit();
                self.text_renderer.cache_x = 0;
                self.text_renderer.cache_y = 0;
                self.text_renderer.cache_row_height = 0;

                info!("Now playing: {} - {}", track.artist, track.title);
                let has_art = track.album_art.is_some();
                // take() strips the massive image payload out of TrackInfo so we don't hoard it in RAM permanently!
                if let Some(art) = track.album_art.take() {
                    info!(
                        "Track contains album art ({} bytes raw). Sending to GPU...",
                        (art.len() as wgpu::BufferAddress)
                    );
                    self.update_album_art_texture(&art);
                } else {
                    warn!("Track event received, but album_art payload is None!");
                    self.album_art_bg_bind_group = None;
                    self.album_art_fg_bind_group = None;
                    self.current_album_texture = None;
                }
                self.state.has_album_art = has_art;
                self.cached_track_str =
                    format!("{} — {}\n{}", track.title, track.artist, track.album);
                self.cached_track_hash = hash_str(&self.cached_track_str);
                self.state.previous_palette = self
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.clone());
                self.state.current_track = Some(*track);
                self.update_theme_colors();
                self.update_text_colors();
                self.state.is_playing = true;
                self.current_lyric_idx = 0;
                self.lyric_scroll_offset = 0.0;
                self.state.begin_transition();
            }

            Event::PlaybackStopped => {
                self.state.is_playing = false;
                // We intentionally do not clear the track here so it remains visible while paused
            }

            Event::PlaybackResumed => {
                self.state.is_playing = true;
            }

            Event::BackgroundVideoFrame(frame) => {
                self.update_background_video_frame(&frame);
            }

            Event::CanvasVideoFrame(frame) => {
                self.update_canvas_video_frame(&frame);
            }

            Event::PlayerShutDown => {
                self.cached_track_str.clear();
                self.cached_track_hash = 0;
                self.text_buffer_cache.clear();
                self.text_buffer_cache.shrink_to_fit();
                self.state.previous_palette = self
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.clone());
                self.album_art_bg_bind_group = None;
                self.album_art_fg_bind_group = None;
                self.current_album_texture = None;
                self.state.has_album_art = false;
                self.state.current_track = None;
                self.update_theme_colors();
                self.update_text_colors();
                self.state.is_playing = false;
                self.current_lyric_idx = 0;
                self.lyric_scroll_offset = 0.0;
                self.state.begin_transition();

                // Free the padding buffers back to the OS allocator on idle
                self.video_frame_buffer.clear();
                self.video_frame_buffer.shrink_to_fit();
                self.album_art_pad_buffer.clear();
                self.album_art_pad_buffer.shrink_to_fit();
            }

            Event::PlaybackPosition(pos) => {
                self.state.playback_position = pos;
            }

            Event::AudioFrame { bands, waveform } => {
                let smoothing = self.state.config.audio.smoothing;
                let inv_smoothing = 1.0 - smoothing;
                let target_len = self.state.audio_bands.len();

                let bands_len = bands.len();

                // --- Smart Beat Detection ---
                // We focus strictly on the low-end frequencies (e.g. 20Hz - 120Hz)
                // Using pre-calculated ranges to avoid redundant math.
                let (bass_min, bass_max) = self.bass_bin_range;
                let bass_slice = &bands[bass_min..=bass_max.min(bands_len.saturating_sub(1))];

                let current_bass = if !bass_slice.is_empty() {
                    bass_slice.iter().sum::<f32>() / bass_slice.len() as f32
                } else {
                    0.0
                };

                // Moving average for a local bass energy threshold (~1 second tracker)
                self.bass_moving_average = self.bass_moving_average * 0.95 + current_bass * 0.05;

                // Trigger a beat if the bass spikes significantly above the recent average
                if current_bass > self.bass_moving_average * 1.3
                    && current_bass > 0.005
                    && self.last_beat_time.elapsed().as_millis() > 200
                {
                    // 200ms cooldown prevents double-triggering
                    self.beat_pulse = 1.0;

                    // Add physical velocity to the lyric spring. The harder the bass spike, the bigger the bounce!
                    let spike =
                        (current_bass / self.bass_moving_average.max(0.001)).clamp(1.2, 3.0);
                    self.lyric_bounce_velocity += (15.0 * spike) * self.theme.effects.lyric_bounce;
                    self.last_beat_time = Instant::now();
                }

                // --- Smart Treble Detection (Snares / Hi-Hats) ---
                let (treble_min, treble_max) = self.treble_bin_range;
                let treble_slice = &bands[treble_min..=treble_max.min(bands_len.saturating_sub(1))];

                let current_treble = if !treble_slice.is_empty() {
                    treble_slice.iter().sum::<f32>() / treble_slice.len() as f32
                } else {
                    0.0
                };

                self.treble_moving_average =
                    self.treble_moving_average * 0.90 + current_treble * 0.10;

                if current_treble > self.treble_moving_average * 1.2
                    && current_treble > 0.002
                    && self.last_treble_time.elapsed().as_millis() > 50
                {
                    // Fast 50ms cooldown for rapid 16th-note hi-hats
                    self.treble_pulse = 1.0;
                    self.last_treble_time = Instant::now();
                }

                let mut total_energy = 0.0;
                // Optimization: Use zipped iterators instead of manual indexing
                // to eliminate bounds checking and enable auto-vectorization.
                for (current, (&(bin_lo, bin_hi), &a_weighting_norm)) in
                    self.state.audio_bands.iter_mut().zip(
                        self.frequency_bin_ranges
                            .iter()
                            .zip(&self.a_weighting_curve),
                    )
                {
                    let max_val =
                        bands
                            .get(bin_lo..bin_hi.min(bands_len))
                            .map_or(0.0, |slice: &[f32]| {
                                slice
                                    .iter()
                                    .fold(0.0f32, |acc, &val| if val > acc { val } else { acc })
                            });

                    let target = (max_val * a_weighting_norm * 2.5).clamp(0.0, 1.0);

                    // Optimization: Use more efficient lerp formula a + (b - a) * t
                    // and use pre-calculated inv_smoothing.
                    let diff = target - *current;
                    if target > *current {
                        *current += diff * 0.8;
                    } else {
                        *current += diff * inv_smoothing;
                    }
                    total_energy += *current;
                }

                // Optimization: Calculate audio_base_energy during the bands loop to avoid a second pass.
                if target_len > 0 {
                    let avg_energy = total_energy / target_len as f32;
                    self.audio_base_energy = avg_energy * 5.0;
                    // Optimization: Cache the average audio energy to make SceneHint detection O(1) in the hot path.
                    self.state.audio_energy = avg_energy;
                } else {
                    self.audio_base_energy = 0.0;
                    self.state.audio_energy = 0.0;
                }

                if self.state.audio_waveform.len() != target_len {
                    self.state.audio_waveform = vec![0.0; target_len].into_boxed_slice();
                }

                let wave_len = waveform.len();
                let mut max_energy = 0.0f32;
                // Optimization: Use zipped iterators for the waveform smoothing loop.
                for (current, &(start, end)) in self
                    .state
                    .audio_waveform
                    .iter_mut()
                    .zip(self.waveform_bin_ranges.iter())
                {
                    let mut peak = 0.0f32;
                    let mut peak_abs = 0.0f32;
                    if let Some(slice) = waveform.get(start..end.min(wave_len)) {
                        for &val in slice {
                            let val_abs: f32 = val.abs();
                            if val_abs > peak_abs {
                                peak_abs = val_abs;
                                peak = val;
                            }
                        }
                    }

                    // Optimization: Track max absolute energy during the waveform loop to avoid a separate pass.
                    if peak_abs > max_energy {
                        max_energy = peak_abs;
                    }

                    *current += (peak - *current) * inv_smoothing;
                }
                self.audio_max_energy = max_energy;
            }

            Event::WeatherUpdated(weather) => {
                info!(
                    "Weather: {:?} {:.1}°C",
                    weather.condition, weather.temperature_celsius
                );
                self.state.weather = Some(*weather);
                self.update_weather_string();
                self.state.begin_transition();
            }
        }
    }
}
