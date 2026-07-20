use super::*;
impl Renderer {
    pub(crate) async fn handle_event(&mut self, event: Event) {
        use crate::modules::renderer::utils::hash_str;
        match event {
            Event::ConfigUpdated(config, theme_layout) => {
                let _ = self.show_lyrics_tx.send(config.audio.show_lyrics);

                // A background video streams its frames into the custom
                // background texture, so when it turns off the wallpaper must
                // be reloaded even though the resolved background itself is
                // unchanged - otherwise the last decoded frame stays stuck.
                let video_stopped = self.state.config.appearance.video_background_path.is_some()
                    && config.appearance.video_background_path.is_none();
                let new_bg = config.appearance.resolved_background().await;
                if new_bg != self.current_bg || video_stopped {
                    self.load_resolved_background(new_bg.as_ref());
                    self.current_bg = new_bg;
                }

                if config.audio.bands != self.state.config.audio.bands {
                    self.audio.reconfigure_bands(config.audio.bands);
                    self.state.audio_energy = 0.0;
                }

                // Always reload the shader pipeline so live WGSL edits apply instantly!
                // self.surface_format, not self.outputs[0]: outputs can be empty
                // while every monitor is disconnected, and indexing it here
                // crashed the engine on any config reload during that window.
                let format = self.surface_format;
                self.visualiser_pass
                    .reload(
                        &self.device,
                        format,
                        &config.audio.style,
                        config.audio.bands,
                    )
                    .await;

                let blur_settings_changed = config.appearance.disable_blur
                    != self.state.config.appearance.disable_blur
                    || config.appearance.blur_opacity != self.state.config.appearance.blur_opacity
                    || config.appearance.album_art_background
                        != self.state.config.appearance.album_art_background;

                // Always reload the theme layout so live edits to the .toml apply instantly!
                self.theme = *theme_layout;
                self.audio.set_smoothing(config.audio.smoothing);
                self.state.config = *config;
                self.update_theme_colors();

                if blur_settings_changed {
                    self.refresh_blur_chains();
                }

                // Optimization: Clear the text buffer cache on config updates to ensure
                // changes like font family or size are applied immediately.
                // We omit shrink_to_fit() to preserve allocated capacity for subsequent lyrics.
                self.text_buffer_cache.clear();

                self.is_waveform_style = self.state.config.audio.style == "waveform";
                self.update_weather_state();
                self.update_weather_string();
                info!("Live settings applied!");
            }
            Event::TrackChanged(mut track) => {
                self.text_buffer_cache.clear(); // Free old shaped lyrics from memory!

                // Optimization: Don't shrink staging buffers to fit on track changes;
                // keep the allocations ready for the next track's album art or video loops.
                // Recreate SwashCache to flush its internal rasterized glyph memory
                self.swash_cache = SwashCache::new();
                self.text_renderer.glyph_cache.clear();
                self.text_renderer.cache_x = 0;
                self.text_renderer.cache_y = 0;
                self.text_renderer.cache_row_height = 0;

                info!("Now playing: {} - {}", track.artist, track.title);
                // take() strips the massive image payload out of TrackInfo so we don't hoard it in RAM permanently!
                if let Some(art) = track.album_art.take() {
                    info!(
                        "Track contains album art ({} bytes raw). Sending to GPU...",
                        (art.len() as wgpu::BufferAddress)
                    );
                    self.update_album_art_texture(&art);
                    self.state.has_album_art = true;
                    self.pending_art_deadline = None;
                } else {
                    // The art is still being fetched in the background and will
                    // arrive via TrackAssetsLoaded. Keep the previous track's art
                    // and colours on screen meanwhile - clearing them here made
                    // every track change flash the no-media scene - but set a
                    // deadline so they fade out if nothing ever arrives.
                    // Grace period matches the HTTP client's 10s timeout: any fetch
                    // that will succeed at all lands within it, so the fade only
                    // triggers on genuine failures (which also fade themselves out
                    // early via an art-less TrackAssetsLoaded).
                    info!("Track event received without album art; keeping previous art while it loads");
                    self.pending_art_deadline =
                        Some(Instant::now() + std::time::Duration::from_secs(10));
                    if track.palette.is_none() {
                        track.palette = self
                            .state
                            .current_track
                            .as_ref()
                            .and_then(|t| t.palette.clone());
                    }
                }
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
                // is_playing is deliberately NOT set here: track metadata can
                // change while the player is paused (playlist auto-advance,
                // session restore), and since the playback status doesn't
                // transition, no corrective Status event would follow - the
                // clock would tick and the lyrics would scroll while the music
                // is actually stopped. PlaybackResumed/PlaybackStopped are the
                // only sources of playing state.
                self.current_lyric_idx = 0;
                self.lyric_scroll_offset = 0.0;
                self.state.begin_transition();
            }

            Event::TrackAssetsLoaded(mut track) => {
                // Drop stale results: the track may have changed again while the
                // network fetch behind this event was still in flight.
                let matches_current = self.state.current_track.as_ref().is_some_and(|t| {
                    t.title == track.title && t.artist == track.artist && t.album == track.album
                });
                if matches_current {
                    if let Some(art) = track.album_art.take() {
                        info!(
                            "Late album art arrived for current track ({} bytes raw). Sending to GPU...",
                            (art.len() as wgpu::BufferAddress)
                        );
                        self.update_album_art_texture(&art);
                        self.state.has_album_art = true;
                        self.pending_art_deadline = None;
                    } else if self.pending_art_deadline.is_some() {
                        // The fetch chain concluded without any art: start the
                        // fade-out now instead of waiting out the grace period.
                        self.pending_art_deadline = Some(Instant::now());
                    }
                    let mut palette_updated = false;
                    if let Some(current) = self.state.current_track.as_mut() {
                        if track.palette.is_some() {
                            self.state.previous_palette = current.palette.take();
                            current.palette = track.palette.take();
                            palette_updated = true;
                        }
                        if track.lyrics.is_some() {
                            current.lyrics = track.lyrics.take();
                        }
                        if track.video_url.is_some() {
                            current.video_url = track.video_url.take();
                        }
                    }
                    if palette_updated {
                        self.update_theme_colors();
                        self.update_text_colors();
                        // Fade towards the new palette instead of hard-popping colors
                        self.state.begin_transition();
                    }
                }
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
                // Instant gate: a decoder spawned before the setting was
                // turned off may still be streaming frames.
                if self.state.config.appearance.prefer_canvas {
                    self.update_canvas_video_frame(&frame);
                }
            }

            Event::PlayerShutDown => {
                self.pending_art_deadline = None;
                self.cached_track_str.clear();
                self.cached_track_hash = 0;
                self.text_buffer_cache.clear();
                self.state.previous_palette = self
                    .state
                    .current_track
                    .as_ref()
                    .and_then(|t| t.palette.clone());
                self.album_art_bg_bind_group = None;
                self.album_art_fg_bind_group = None;
                self.current_album_texture = None;
                self.current_album_size = None;
                self.album_blur_chain = None;
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
                // Beat/treble detection, band smoothing, and waveform peak
                // tracking all live in AudioAnalysis; a detected beat is
                // reported back as data rather than reaching into the theme
                // itself, since the lyric-bounce spring it kicks is themed
                // and owned here.
                let result = self.audio.ingest(&bands, &waveform);
                self.state.audio_energy = result.avg_energy;
                if let Some(spike) = result.beat_spike {
                    self.lyric_bounce_velocity += (15.0 * spike) * self.theme.effects.lyric_bounce;
                }
            }

            Event::WeatherUpdated(weather) => {
                info!(
                    "Weather: {:?} {:.1}°C",
                    weather.condition, weather.temperature_celsius
                );
                self.state.weather = Some(*weather);
                self.update_weather_state();
                self.update_weather_string();
                self.state.begin_transition();
            }
        }
    }
}
