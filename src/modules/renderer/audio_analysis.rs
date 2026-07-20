//! Frame-rate audio analysis: FFT-band smoothing, waveform peak tracking,
//! and beat/treble pulse detection (phase 2 of
//! PLAN-renderer-decomposition.md).
//!
//! Owns everything the raw `Event::AudioFrame` data passes through before
//! it reaches a shader: the per-band bin ranges and A-weighting derived
//! from the configured band count, the smoothed output buffers the
//! visualiser reads every frame, and the beat/treble detectors' moving
//! averages and cooldown timers. None of it touches GPU resources.
//!
//! The one piece of coupling this module does *not* own: a detected beat
//! also kicks the lyric-bounce spring, which is themed (`effects.lyric_bounce`)
//! and lives on `Renderer`. `ingest` reports the beat as data
//! (`IngestResult::beat_spike`) rather than reaching into the theme itself,
//! so this module's only job stays "what did the audio do".

use std::time::Instant;

/// Sample rate and FFT window PipeWire/`AudioCapture` are configured for
/// (see `audio.rs`); the bass/treble bin ranges are derived from these and
/// are independent of the configured *band* count.
const SAMPLE_RATE: f32 = 48000.0;
const FFT_SIZE: f32 = 2048.0;

pub(crate) struct IngestResult {
    /// Average smoothed band energy this frame, 0.0 when there are no bands
    /// configured. Callers feed this into `AppState::audio_energy` for
    /// scene-hint classification.
    pub(crate) avg_energy: f32,
    /// `Some(spike)` when a beat was detected this frame, `spike` being how
    /// far the bass spiked above its recent average (clamped 1.2..=3.0) -
    /// the input the lyric-bounce spring's kick is scaled by.
    pub(crate) beat_spike: Option<f32>,
}

pub(crate) struct AudioAnalysis {
    /// Per-band (FFT bin range, A-weighting) derived from the configured
    /// band count; see `renderer::utils::build_audio_processing_bins`.
    processing_bins: Vec<(usize, usize, f32)>,
    /// Per-band waveform sample ranges; see
    /// `renderer::utils::build_waveform_bin_ranges`.
    waveform_bin_ranges: Vec<(usize, usize)>,
    inv_smoothing: f32,
    inv_target_len: f32,
    /// Fixed FFT-bin ranges for beat/treble detection - unlike
    /// `processing_bins`, these depend only on `SAMPLE_RATE`/`FFT_SIZE`, so
    /// they are computed once in `new` and never revisited.
    bass_bin_range: (usize, usize),
    treble_bin_range: (usize, usize),

    /// Smoothed per-band output the visualiser shader binds as its bands
    /// storage buffer.
    pub(crate) bands: Box<[f32]>,
    /// Smoothed per-band waveform peaks, for the waveform visualiser style.
    pub(crate) waveform: Box<[f32]>,

    bass_moving_average: f32,
    pub(crate) beat_pulse: f32,
    last_beat_time: Instant,
    treble_moving_average: f32,
    pub(crate) treble_pulse: f32,
    last_treble_time: Instant,

    /// Peak absolute waveform energy this frame; gates whether the
    /// visualiser is considered "active" at all.
    pub(crate) max_energy: f32,
    /// `avg_energy * 5.0` - the scaled term `draw_frame` blends with the
    /// treble pulse for the album-art audio-reactive scale.
    pub(crate) base_energy: f32,
}

/// (processing bins, waveform bin ranges, `1/band_count`) - the trio
/// [`build_bins`] derives from a band count.
type BandBins = (Vec<(usize, usize, f32)>, Vec<(usize, usize)>, f32);

/// Rebuilds everything that depends on the configured band count: the bin
/// ranges plus the `1/band_count` normalisation factor. Shared by `new` and
/// `reconfigure_bands` so the two can't drift.
fn build_bins(band_count: usize) -> BandBins {
    let processing_bins = super::utils::build_audio_processing_bins(band_count);
    let waveform_bin_ranges = super::utils::build_waveform_bin_ranges(band_count);
    let inv_target_len = if band_count > 0 {
        1.0 / band_count as f32
    } else {
        0.0
    };
    (processing_bins, waveform_bin_ranges, inv_target_len)
}

impl AudioAnalysis {
    pub(crate) fn new(band_count: usize, smoothing: f32) -> Self {
        let (processing_bins, waveform_bin_ranges, inv_target_len) = build_bins(band_count);
        let freq_per_bin = SAMPLE_RATE / FFT_SIZE;

        Self {
            processing_bins,
            waveform_bin_ranges,
            inv_smoothing: 1.0 - smoothing,
            inv_target_len,
            bass_bin_range: (
                (20.0 / freq_per_bin).floor() as usize,
                (120.0 / freq_per_bin).ceil() as usize,
            ),
            treble_bin_range: (
                (3000.0 / freq_per_bin).floor() as usize,
                (8000.0 / freq_per_bin).ceil() as usize,
            ),
            bands: vec![0.0; band_count].into_boxed_slice(),
            waveform: vec![0.0; band_count].into_boxed_slice(),
            bass_moving_average: 0.0,
            beat_pulse: 0.0,
            last_beat_time: Instant::now(),
            treble_moving_average: 0.0,
            treble_pulse: 0.0,
            last_treble_time: Instant::now(),
            max_energy: 0.0,
            base_energy: 0.0,
        }
    }

    /// Rebuilds the bin ranges and zeroes the output buffers for a new band
    /// count. Only call when the count actually changed - the caller (the
    /// `ConfigUpdated` handler) already gates on that, since this always
    /// reallocates.
    pub(crate) fn reconfigure_bands(&mut self, band_count: usize) {
        let (processing_bins, waveform_bin_ranges, inv_target_len) = build_bins(band_count);
        self.processing_bins = processing_bins;
        self.waveform_bin_ranges = waveform_bin_ranges;
        self.inv_target_len = inv_target_len;
        self.bands = vec![0.0; band_count].into_boxed_slice();
        self.waveform = vec![0.0; band_count].into_boxed_slice();
    }

    /// Smoothing can change independently of band count, so this is called
    /// unconditionally on every config update.
    pub(crate) fn set_smoothing(&mut self, smoothing: f32) {
        self.inv_smoothing = 1.0 - smoothing;
    }

    /// Exponential decay of the beat/treble pulses, run once per render-loop
    /// tick regardless of whether an `AudioFrame` arrived that tick (the
    /// pulses fade out through silence, not just between frames of audio).
    /// Flushes to exact 0.0 below a subnormal-float threshold: subnormals
    /// degrade FPU throughput, and 0.0 is indistinguishable from "decayed
    /// out" for every reader (`scene_is_animating`, the visualiser uniform).
    pub(crate) fn decay(&mut self, delta: f32) {
        // Treble decays slightly faster (15 vs 12) for snappier, rapid
        // hi-hats; the beat pulse should still read clearly as it falls.
        let decay_12 = (-12.0 * delta).exp();
        let decay_15 = (-15.0 * delta).exp();
        self.beat_pulse *= decay_12;
        self.treble_pulse *= decay_15;
        if self.beat_pulse.abs() < 1e-5 {
            self.beat_pulse = 0.0;
        }
        if self.treble_pulse.abs() < 1e-5 {
            self.treble_pulse = 0.0;
        }
    }

    /// Processes one raw `AudioFrame`: beat/treble detection against the
    /// fixed FFT-bin ranges, then smooths `raw_bands`/`raw_waveform` (FFT
    /// resolution, up to 1024 bins) down into the configured band count.
    pub(crate) fn ingest(&mut self, raw_bands: &[f32], raw_waveform: &[f32]) -> IngestResult {
        let target_len = self.bands.len();
        let bands_len = raw_bands.len();

        // --- Smart Beat Detection ---
        // Focuses strictly on the low-end frequencies (e.g. 20-120Hz).
        let (bass_min, bass_max) = self.bass_bin_range;
        let bass_slice = &raw_bands[bass_min..=bass_max.min(bands_len.saturating_sub(1))];
        let current_bass = if bass_slice.is_empty() {
            0.0
        } else {
            bass_slice.iter().sum::<f32>() / bass_slice.len() as f32
        };

        // Moving average for a local bass energy threshold (~1 second tracker).
        self.bass_moving_average = self.bass_moving_average * 0.95 + current_bass * 0.05;

        // Trigger a beat if the bass spikes significantly above the recent
        // average; the 200ms cooldown prevents double-triggering.
        let beat_spike = if current_bass > self.bass_moving_average * 1.3
            && current_bass > 0.005
            && self.last_beat_time.elapsed().as_millis() > 200
        {
            self.beat_pulse = 1.0;
            let spike = (current_bass / self.bass_moving_average.max(0.001)).clamp(1.2, 3.0);
            self.last_beat_time = Instant::now();
            Some(spike)
        } else {
            None
        };

        // --- Smart Treble Detection (Snares / Hi-Hats) ---
        let (treble_min, treble_max) = self.treble_bin_range;
        let treble_slice = &raw_bands[treble_min..=treble_max.min(bands_len.saturating_sub(1))];
        let current_treble = if treble_slice.is_empty() {
            0.0
        } else {
            treble_slice.iter().sum::<f32>() / treble_slice.len() as f32
        };

        self.treble_moving_average = self.treble_moving_average * 0.90 + current_treble * 0.10;

        // Fast 50ms cooldown for rapid 16th-note hi-hats.
        if current_treble > self.treble_moving_average * 1.2
            && current_treble > 0.002
            && self.last_treble_time.elapsed().as_millis() > 50
        {
            self.treble_pulse = 1.0;
            self.last_treble_time = Instant::now();
        }

        let mut total_energy = 0.0;
        // Zipped iterators eliminate bounds checking and enable
        // auto-vectorization over the (typically 64-band) smoothing loop.
        for (current, &(bin_lo, bin_hi, combined_weight)) in
            self.bands.iter_mut().zip(self.processing_bins.iter())
        {
            let mut max_val = 0.0f32;
            if let Some(slice) = raw_bands.get(bin_lo..bin_hi.min(bands_len)) {
                for &val in slice {
                    if val > max_val {
                        max_val = val;
                    }
                }
            }

            let target = (max_val * combined_weight).clamp(0.0, 1.0);
            let diff = target - *current;
            if target > *current {
                *current += diff * 0.8;
            } else {
                *current += diff * self.inv_smoothing;
            }
            total_energy += *current;
        }

        let avg_energy = if target_len > 0 {
            let avg = total_energy * self.inv_target_len;
            self.base_energy = avg * 5.0;
            avg
        } else {
            self.base_energy = 0.0;
            0.0
        };

        // Defensive: keeps the waveform buffer in sync with the configured
        // band count even if it somehow drifted out of step with `bands`.
        if self.waveform.len() != target_len {
            self.waveform = vec![0.0; target_len].into_boxed_slice();
        }

        let wave_len = raw_waveform.len();
        let mut max_energy = 0.0f32;
        for (current, &(start, end)) in self
            .waveform
            .iter_mut()
            .zip(self.waveform_bin_ranges.iter())
        {
            let mut peak = 0.0f32;
            let mut peak_abs = 0.0f32;
            if let Some(slice) = raw_waveform.get(start..end.min(wave_len)) {
                for &val in slice {
                    let val_abs = val.abs();
                    if val_abs > peak_abs {
                        peak_abs = val_abs;
                        peak = val;
                    }
                }
            }
            if peak_abs > max_energy {
                max_energy = peak_abs;
            }
            *current += (peak - *current) * self.inv_smoothing;
        }
        self.max_energy = max_energy;

        IngestResult {
            avg_energy,
            beat_spike,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Raw FFT-resolution buffer, quiet everywhere except the bass bins
    /// this `AudioAnalysis`'s bass_bin_range covers (0..=6 at the fixed
    /// 48kHz/2048 configuration).
    fn loud_bass_frame() -> Vec<f32> {
        let mut frame = vec![0.0f32; 512];
        for v in &mut frame[0..=6] {
            *v = 1.0;
        }
        frame
    }

    #[test]
    fn reconfigure_bands_rebuilds_ranges_and_resizes_buffers() {
        let mut audio = AudioAnalysis::new(64, 0.7);
        assert_eq!(audio.bands.len(), 64);
        assert_eq!(audio.processing_bins.len(), 64);

        audio.reconfigure_bands(32);

        assert_eq!(audio.bands.len(), 32);
        assert_eq!(audio.waveform.len(), 32);
        assert_eq!(audio.processing_bins.len(), 32);
        assert_eq!(audio.waveform_bin_ranges.len(), 32);
        assert!((audio.inv_target_len - 1.0 / 32.0).abs() < 1e-6);
    }

    #[test]
    fn set_smoothing_updates_inv_smoothing_independent_of_bands() {
        let mut audio = AudioAnalysis::new(64, 0.7);
        assert!((audio.inv_smoothing - 0.3).abs() < 1e-6);
        audio.set_smoothing(0.5);
        assert!((audio.inv_smoothing - 0.5).abs() < 1e-6);
        // Band buffers are untouched by a smoothing-only change.
        assert_eq!(audio.bands.len(), 64);
    }

    #[test]
    fn bass_impulse_triggers_one_beat_pulse_with_cooldown() {
        let mut audio = AudioAnalysis::new(8, 0.7);
        let quiet = vec![0.0f32; 512];
        let loud = loud_bass_frame();

        // Warm up the moving average on silence so the impulse reads as a
        // genuine spike, the way a real quiet intro would.
        for _ in 0..5 {
            let r = audio.ingest(&quiet, &quiet);
            assert!(r.beat_spike.is_none());
        }

        // `last_beat_time` starts at construction time (matching production:
        // `Renderer::new` sets it the same way), so a fast test running
        // within 200ms of `new()` would otherwise find the cooldown already
        // blocking the very first beat.
        std::thread::sleep(std::time::Duration::from_millis(210));

        let r = audio.ingest(&loud, &quiet);
        assert!(r.beat_spike.is_some());
        assert_eq!(audio.beat_pulse, 1.0);

        // Still inside the 200ms real-time cooldown: back-to-back impulses
        // must not double-trigger.
        let r2 = audio.ingest(&loud, &quiet);
        assert!(r2.beat_spike.is_none());
    }

    #[test]
    fn decay_flushes_subnormal_pulses_to_exact_zero() {
        let mut audio = AudioAnalysis::new(8, 0.7);
        audio.beat_pulse = 1e-6;
        audio.treble_pulse = 1e-6;
        audio.decay(0.016);
        assert_eq!(audio.beat_pulse, 0.0);
        assert_eq!(audio.treble_pulse, 0.0);
    }

    #[test]
    fn decay_shrinks_pulses_geometrically_before_flushing() {
        let mut audio = AudioAnalysis::new(8, 0.7);
        audio.beat_pulse = 1.0;
        audio.treble_pulse = 1.0;
        audio.decay(1.0 / 60.0);
        assert!(audio.beat_pulse > 0.0 && audio.beat_pulse < 1.0);
        // Treble's faster decay constant (15 vs 12) means it falls further
        // in the same tick.
        assert!(audio.treble_pulse > 0.0 && audio.treble_pulse < audio.beat_pulse);
    }

    #[test]
    fn zero_band_count_yields_zero_energy_without_panicking() {
        let mut audio = AudioAnalysis::new(0, 0.7);
        let raw = vec![0.0f32; 512];
        let r = audio.ingest(&raw, &raw);
        assert_eq!(r.avg_energy, 0.0);
        assert_eq!(audio.base_energy, 0.0);
    }
}
