use anyhow::Result;
use ffmpeg_next as ffmpeg;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::mpsc::Sender;
use tracing::{info, warn};

use super::event::Event;

mod hwaccel;

pub struct PooledImage {
    img: Option<image::RgbaImage>,
    recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl PooledImage {
    pub fn new(img: image::RgbaImage, recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            img: Some(img),
            recycle_tx,
        }
    }

    // Keeps backwards compatibility if the renderer manually consumes the raw buffer
    pub fn into_raw(mut self) -> Vec<u8> {
        self.img.take().map(|i| i.into_raw()).unwrap_or_default()
    }
}

impl std::ops::Deref for PooledImage {
    type Target = image::RgbaImage;
    fn deref(&self) -> &Self::Target {
        // Using a static default image guarantees no panic if dereferenced after drop
        static DEFAULT_IMG: std::sync::LazyLock<image::RgbaImage> =
            std::sync::LazyLock::new(|| image::RgbaImage::new(1, 1));
        self.img.as_ref().unwrap_or(&DEFAULT_IMG)
    }
}

impl Drop for PooledImage {
    fn drop(&mut self) {
        if let Some(img) = self.img.take() {
            let _ = self.recycle_tx.try_send(img.into_raw());
        }
    }
}

impl std::fmt::Debug for PooledImage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("PooledImage").finish()
    }
}

/// Copies a scaled ffmpeg video frame's plane 0 into `dst`, taking a fast
/// bulk-`memcpy` path when the frame is densely packed (stride == row width)
/// and falling back to a row-by-row copy when ffmpeg has padded each row.
fn copy_scaled_frame(
    dst: &mut [u8],
    rgb_frame: &ffmpeg::frame::Video,
    width: u32,
    frame_size: usize,
) {
    let stride = rgb_frame.stride(0);
    let data = rgb_frame.data(0);
    let expected_row_bytes = (width * 4) as usize;

    if stride == expected_row_bytes {
        dst[..frame_size].copy_from_slice(&data[..frame_size]);
    } else {
        for (dst_row, src_row) in dst[..frame_size]
            .chunks_exact_mut(expected_row_bytes)
            .zip(data.chunks(stride))
        {
            dst_row.copy_from_slice(&src_row[..expected_row_bytes]);
        }
    }
}

/// Everything the local decode loop derives from one opened file.
struct LocalStream {
    ictx: ffmpeg::format::context::Input,
    stream_index: usize,
    decoder: ffmpeg::decoder::Video,
    time_base_f64: f64,
    /// Whether the decoder was set up for VAAPI. Individual frames can
    /// still arrive in software if the GPU turns the stream down.
    hardware: bool,
}

/// Opens `path` fresh and derives everything the local decode loop needs
/// from it: the demuxer context, which stream is video, a decoder matched
/// to that stream's own parameters, and its time base. Used both for the
/// loop's initial setup and to fully reinitialize after a failed
/// seek-to-start - a bare reopen of just `ictx` used to leave the decoder,
/// stream index and time base all still pointing at state derived from the
/// *previous* `Input`, which a fresh one isn't guaranteed to match
/// (ffmpeg's C API gives no guarantee a codec context stays valid once the
/// format context that produced its parameters is gone).
///
/// With a `vaapi` device, the decoder decodes on the GPU when the codec
/// has a VAAPI path.
fn open_video_stream(
    path: &str,
    vaapi: Option<&hwaccel::VaapiDevice>,
) -> Result<LocalStream, String> {
    let ictx = ffmpeg::format::input(&path).map_err(|e| format!("failed to open input: {e}"))?;

    let input = ictx
        .streams()
        .best(ffmpeg::media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)
        .map_err(|e| format!("failed to find video stream: {e}"))?;
    let stream_index = input.index();

    let mut context_decoder = ffmpeg::codec::context::Context::from_parameters(input.parameters())
        .map_err(|e| format!("failed to get codec context: {e}"))?;

    let codec = ffmpeg::decoder::find(context_decoder.id());
    let codec_name = codec.as_ref().map_or("unknown", |c| c.name()).to_string();
    let hardware = match (vaapi, &codec) {
        (Some(device), Some(codec)) if hwaccel::codec_supports_vaapi(codec) => {
            match hwaccel::attach(&mut context_decoder, device) {
                Ok(()) => true,
                Err(e) => {
                    warn!("Could not attach VAAPI to the {codec_name} decoder: {e}");
                    false
                }
            }
        }
        (Some(_), _) => {
            info!("No VAAPI decoder for {codec_name}; decoding in software");
            false
        }
        (None, _) => false,
    };

    let decoder = context_decoder
        .decoder()
        .video()
        .map_err(|e| format!("failed to get video decoder: {e}"))?;
    if hardware {
        info!(
            "Decoding {codec_name} {}x{} video with VAAPI",
            decoder.width(),
            decoder.height()
        );
    }

    let time_base = input.time_base();
    let time_base_f64 = time_base.numerator() as f64 / time_base.denominator() as f64;

    Ok(LocalStream {
        ictx,
        stream_index,
        decoder,
        time_base_f64,
        hardware,
    })
}

/// The size to convert a `src`-sized video to so it still covers a
/// `target` (the largest monitor, in physical pixels): scaled down with its
/// aspect ratio kept, never scaled up. The renderer crops/fits the texture
/// to each screen afterwards, so anything larger than this is converted,
/// copied and uploaded only to be thrown away by the GPU sampler. `None`
/// (monitor size not known yet) keeps the source size.
pub(crate) fn scaled_video_size(src: (u32, u32), target: Option<(u32, u32)>) -> (u32, u32) {
    let Some((tw, th)) = target.filter(|&(w, h)| w > 0 && h > 0) else {
        return src;
    };
    if src.0 == 0 || src.1 == 0 {
        return src;
    }
    let scale = (tw as f64 / src.0 as f64).max(th as f64 / src.1 as f64);
    if scale >= 1.0 {
        return src;
    }
    // Round up so the result never falls short of covering the target.
    let w = ((src.0 as f64 * scale).ceil() as u32).clamp(1, src.0);
    let h = ((src.1 as f64 * scale).ceil() as u32).clamp(1, src.1);
    (w, h)
}

/// The swscale `SWS_CS_*` matrix for converting a video tagged
/// `space` to RGB. Untagged video (common in the wild) gets the same guess
/// mpv and most players make: BT.709 for HD (720 lines and up), BT.601
/// below - swscale's own default is BT.601 regardless, which shifts the
/// colours of the typical untagged HD file.
pub(crate) fn sws_colorspace(space: ffmpeg::color::Space, height: u32) -> std::ffi::c_int {
    use ffmpeg::color::Space;
    use ffmpeg::ffi::{SWS_CS_BT2020, SWS_CS_FCC, SWS_CS_ITU601, SWS_CS_ITU709, SWS_CS_SMPTE240M};
    match space {
        Space::BT709 => SWS_CS_ITU709,
        Space::FCC => SWS_CS_FCC,
        Space::BT470BG | Space::SMPTE170M => SWS_CS_ITU601,
        Space::SMPTE240M => SWS_CS_SMPTE240M,
        Space::BT2020NCL | Space::BT2020CL => SWS_CS_BT2020,
        _ if height >= 720 => SWS_CS_ITU709,
        _ => SWS_CS_ITU601,
    }
}

/// Everything a scaler is built for. Frames are checked against it, so a
/// change in any of these (a new monitor size, or a stream that switches
/// between hardware and software frames) rebuilds the scaler.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ScalerKey {
    format: ffmpeg::format::Pixel,
    src: (u32, u32),
    target: Option<(u32, u32)>,
    space: ffmpeg::color::Space,
    range: ffmpeg::color::Range,
}

impl ScalerKey {
    fn for_frame(frame: &ffmpeg::frame::Video, target: Option<(u32, u32)>) -> Self {
        Self {
            format: frame.format(),
            src: (frame.width(), frame.height()),
            target,
            space: frame.color_space(),
            range: frame.color_range(),
        }
    }
}

/// The RGBA converter for the local decoder, sized by `scaled_video_size`.
struct FrameScaler {
    ctx: ffmpeg::software::scaling::Context,
    key: ScalerKey,
    width: u32,
    height: u32,
    frame_size: usize,
}

impl FrameScaler {
    fn new(key: ScalerKey) -> Result<Self, String> {
        let (src_w, src_h) = key.src;
        let (width, height) = scaled_video_size(key.src, key.target);
        let mut ctx = ffmpeg::software::scaling::Context::get(
            key.format,
            src_w,
            src_h,
            ffmpeg::format::Pixel::RGBA,
            width,
            height,
            ffmpeg::software::scaling::flag::Flags::BILINEAR,
        )
        .map_err(|e| format!("failed to create scaler: {e}"))?;

        // Left alone, swscale converts every YUV video as limited-range
        // BT.601. Use the stream's own matrix and range instead (see
        // sws_colorspace). Output is full-range RGB, as before.
        let matrix = sws_colorspace(key.space, src_h);
        let full_range = i32::from(key.range == ffmpeg::color::Range::JPEG);
        // SAFETY: `ctx` is a live, initialised SwsContext owned by `ctx`;
        // sws_getCoefficients returns a pointer to a static table for any
        // input (unknown values fall back to the default table).
        let status = unsafe {
            let coefficients = ffmpeg::ffi::sws_getCoefficients(matrix);
            ffmpeg::ffi::sws_setColorspaceDetails(
                ctx.as_mut_ptr(),
                coefficients,
                full_range,
                ffmpeg::ffi::sws_getCoefficients(ffmpeg::ffi::SWS_CS_DEFAULT),
                1,
                0,
                1 << 16,
                1 << 16,
            )
        };
        if status < 0 {
            // Non-YUV sources (e.g. RGB video) have no matrix to set.
            tracing::debug!("swscale kept its default colourspace details ({status})");
        }

        if (width, height) != key.src {
            info!("Scaling {src_w}x{src_h} video to {width}x{height} to match the display");
        }
        Ok(Self {
            ctx,
            key,
            width,
            height,
            frame_size: (width * height * 4) as usize,
        })
    }

    /// The scaler for `frame`: the one in `slot` when it still matches,
    /// otherwise a freshly built one (the `bool` says it was rebuilt).
    fn for_frame<'a>(
        slot: &'a mut Option<FrameScaler>,
        frame: &ffmpeg::frame::Video,
        target: Option<(u32, u32)>,
    ) -> Result<(&'a mut FrameScaler, bool), String> {
        let key = ScalerKey::for_frame(frame, target);
        let rebuild = slot.as_ref().is_none_or(|scaler| scaler.key != key);
        if rebuild {
            *slot = None;
            *slot = Some(FrameScaler::new(key)?);
        }
        Ok((slot.as_mut().expect("scaler was just set"), rebuild))
    }
}

/// What happened to one frame handed to `send_frame`.
enum FrameSent {
    Sent,
    /// Conversion failed, or the renderer's queue was full.
    Dropped,
    /// The renderer has gone away; the decoder should stop.
    Closed,
}

/// Converts one decoded (system-memory) frame to RGBA at display size and
/// hands it to the renderer, reusing a recycled buffer when one is
/// available.
fn send_frame(
    scaler_slot: &mut Option<FrameScaler>,
    target: Option<(u32, u32)>,
    frame: &ffmpeg::frame::Video,
    rgb_frame: &mut ffmpeg::frame::Video,
    recycle_rx: &mut tokio::sync::mpsc::Receiver<Vec<u8>>,
    recycle_tx: &tokio::sync::mpsc::Sender<Vec<u8>>,
    tx: &Sender<Event>,
) -> FrameSent {
    let (scaler, rebuilt) = match FrameScaler::for_frame(scaler_slot, frame, target) {
        Ok(found) => found,
        Err(e) => {
            warn!("Dropping video frame: {e}");
            return FrameSent::Dropped;
        }
    };
    if rebuilt {
        // The previous output frame has the old size/format; let the
        // scaler allocate a matching one.
        *rgb_frame = ffmpeg::frame::Video::empty();
    }
    if scaler.ctx.run(frame, rgb_frame).is_err() {
        return FrameSent::Dropped;
    }
    let frame_size = scaler.frame_size;
    let mut buffer = recycle_rx
        .try_recv()
        .unwrap_or_else(|_| vec![0u8; frame_size]);
    if buffer.len() != frame_size {
        buffer.resize(frame_size, 0);
    }

    copy_scaled_frame(&mut buffer, rgb_frame, scaler.width, frame_size);

    let Some(img) = image::RgbaImage::from_raw(scaler.width, scaler.height, buffer) else {
        return FrameSent::Dropped;
    };
    let pooled_img = Box::new(PooledImage::new(img, recycle_tx.clone()));
    match tx.try_send(Event::BackgroundVideoFrame(pooled_img)) {
        Ok(_) => FrameSent::Sent,
        Err(tokio::sync::mpsc::error::TrySendError::Full(Event::BackgroundVideoFrame(dropped))) => {
            let _ = recycle_tx.try_send(dropped.into_raw());
            FrameSent::Dropped
        }
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => FrameSent::Closed,
        Err(_) => FrameSent::Dropped,
    }
}

/// Consecutive packets the VAAPI decoder may reject before the decoder
/// gives up on the GPU and reopens the file for software decoding.
const MAX_HARDWARE_PACKET_ERRORS: u32 = 8;

/// Turns a decoded frame into one `send_frame` can convert: VAAPI surfaces
/// are downloaded into `system`, software frames pass through. `Err` means
/// the download failed and the decoder should fall back to software.
fn system_frame<'a>(
    decoded: &'a ffmpeg::frame::Video,
    system: &'a mut ffmpeg::frame::Video,
) -> Result<&'a ffmpeg::frame::Video, ffmpeg::Error> {
    if hwaccel::is_vaapi_frame(decoded) {
        hwaccel::download(decoded, system)?;
        Ok(system)
    } else {
        Ok(decoded)
    }
}

/// Blocks the decode thread while the wallpaper is hidden (fully covered,
/// or the monitor is off), so nothing is decoded or converted just to be
/// thrown away. Returns `false` when the decoder should stop instead: it
/// was cancelled, or a sender went away.
fn wait_until_visible(
    handle: &tokio::runtime::Handle,
    visible_rx: &mut tokio::sync::watch::Receiver<bool>,
    cancel_rx: &mut tokio::sync::watch::Receiver<bool>,
) -> bool {
    info!("Wallpaper hidden; pausing video decode");
    let resumed = handle.block_on(async {
        loop {
            if *cancel_rx.borrow_and_update() {
                return false;
            }
            if *visible_rx.borrow_and_update() {
                return true;
            }
            tokio::select! {
                res = visible_rx.changed() => if res.is_err() { return false; },
                res = cancel_rx.changed() => if res.is_err() { return false; },
            }
        }
    });
    if resumed {
        info!("Wallpaper visible again; resuming video decode");
    }
    resumed
}

pub struct VideoDecoder;

impl VideoDecoder {
    /// Decodes a local video file on a blocking thread, looping forever and
    /// streaming frames to the renderer as `BackgroundVideoFrame`s.
    ///
    /// Frames are paced to the file's own timestamps and thinned to the
    /// configured fps; decoding pauses entirely while `visible_rx` reads
    /// false, and frames are converted at the size `size_rx` asks for (see
    /// `scaled_video_size`). When `appearance.hardware_video_decode` is on
    /// and a VAAPI device opens, decoding runs on the GPU, falling back to
    /// software for codecs it can't handle or if it starts failing.
    #[allow(clippy::too_many_arguments)]
    pub async fn run_local_decoder(
        path: String,
        tx: Sender<Event>,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
        config_rx: tokio::sync::watch::Receiver<super::config::Config>,
        mut visible_rx: tokio::sync::watch::Receiver<bool>,
        mut size_rx: tokio::sync::watch::Receiver<Option<(u32, u32)>>,
        mut recycle_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
        recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> Result<()> {
        let _ = ffmpeg::init();
        info!("Starting local ffmpeg-next video decoder for: {}", path);

        let handle = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            let hardware_enabled = config_rx.borrow().appearance.hardware_video_decode;
            let vaapi = if hardware_enabled {
                match hwaccel::VaapiDevice::open() {
                    Ok(device) => Some(device),
                    Err(e) => {
                        info!("VAAPI unavailable ({e}); decoding video in software");
                        None
                    }
                }
            } else {
                info!("Hardware video decoding is turned off; decoding in software");
                None
            };
            // Cleared for good (for this video) if the GPU path fails.
            let mut use_vaapi = vaapi.is_some();

            let mut stream = match open_video_stream(&path, vaapi.as_ref()) {
                Ok(stream) => stream,
                Err(e) => {
                    warn!("ffmpeg-next failed to open {}: {}", path, e);
                    return;
                }
            };

            let mut target = *size_rx.borrow_and_update();
            let mut scaler: Option<FrameScaler> = None;
            // Reused across every frame: scaler.run() only allocates this frame's
            // internal buffer the first time (while it's still empty), so keeping
            // it outside the loop avoids a full-resolution RGBA allocation per frame.
            let mut rgb_frame = ffmpeg::frame::Video::empty();
            // Destination for downloaded VAAPI frames (see hwaccel::download).
            let mut system_buffer = ffmpeg::frame::Video::empty();
            let mut reopen_in_software = false;

            // Loop infinitely
            'outer: loop {
                if *cancel_rx.borrow() {
                    break;
                }

                if reopen_in_software {
                    // A fresh software stream starts at the beginning, so
                    // there's nothing to seek.
                    reopen_in_software = false;
                    use_vaapi = false;
                    match open_video_stream(&path, None) {
                        Ok(new_stream) => stream = new_stream,
                        Err(e) => {
                            warn!("ffmpeg-next reopen for software decoding failed: {}", e);
                            break;
                        }
                    }
                } else if let Err(e) = stream.ictx.seek(0, 0..stream.ictx.duration().max(0)) {
                    // We need to seek to the beginning if we loop.
                    warn!("ffmpeg-next seek failed: {}", e);
                    // Reopening alone isn't enough: the decoder, stream
                    // index and time base were all derived from the
                    // *previous* input (see open_video_stream), so rebuild
                    // everything from the new one together.
                    let device = vaapi.as_ref().filter(|_| use_vaapi);
                    match open_video_stream(&path, device) {
                        Ok(new_stream) => stream = new_stream,
                        Err(e) => {
                            warn!("ffmpeg-next reopen after failed seek also failed: {}", e);
                            break;
                        }
                    }
                }
                stream.decoder.flush();
                let mut first_pts: Option<i64> = None;
                let mut start_time = tokio::time::Instant::now();
                let mut last_sent_time = 0.0;
                let mut hardware_errors = 0u32;

                for (packet_stream, packet) in stream.ictx.packets() {
                    if *cancel_rx.borrow() {
                        break 'outer;
                    }

                    if !*visible_rx.borrow() {
                        let paused_at = tokio::time::Instant::now();
                        if !wait_until_visible(&handle, &mut visible_rx, &mut cancel_rx) {
                            break 'outer;
                        }
                        // Shift the pacing clock past the pause, or playback
                        // would race through every frame "owed" since then.
                        start_time += paused_at.elapsed();
                    }

                    if size_rx.has_changed().unwrap_or(false) {
                        // Picked up by send_frame, which rebuilds the scaler.
                        target = *size_rx.borrow_and_update();
                    }

                    if packet_stream.index() != stream.stream_index {
                        continue;
                    }
                    match stream.decoder.send_packet(&packet) {
                        Ok(()) => hardware_errors = 0,
                        Err(e) if stream.hardware => {
                            hardware_errors += 1;
                            if hardware_errors >= MAX_HARDWARE_PACKET_ERRORS {
                                warn!(
                                    "VAAPI decoder keeps failing ({e}); switching to software decoding"
                                );
                                reopen_in_software = true;
                                continue 'outer;
                            }
                            continue;
                        }
                        Err(_) => continue,
                    }

                    let mut decoded = ffmpeg::frame::Video::empty();
                    while stream.decoder.receive_frame(&mut decoded).is_ok() {
                        if *cancel_rx.borrow() {
                            break 'outer;
                        }

                        let pts = decoded.pts().unwrap_or(0);
                        if first_pts.is_none() {
                            first_pts = Some(pts);
                        }

                        let pts_diff = pts - first_pts.unwrap_or(pts);
                        let target_time = pts_diff as f64 * stream.time_base_f64;
                        let elapsed = start_time.elapsed().as_secs_f64();

                        if target_time > elapsed {
                            let sleep_duration =
                                std::time::Duration::from_secs_f64(target_time - elapsed);
                            std::thread::sleep(sleep_duration);
                        }

                        // Dynamic FPS throttling to save CPU. .max(1) mirrors
                        // Config::sanitise: fps = 0 would make frame_duration
                        // infinite and silently drop every frame.
                        let target_fps = config_rx.borrow().fps.max(1) as f64;
                        let frame_duration = 1.0 / target_fps;

                        // If this frame's target time is less than the duration from the last frame we sent, drop it!
                        // (Before the GPU download, so dropped frames never leave VRAM.)
                        if target_time < last_sent_time + frame_duration && last_sent_time > 0.0 {
                            continue;
                        }

                        let frame = match system_frame(&decoded, &mut system_buffer) {
                            Ok(frame) => frame,
                            Err(e) => {
                                warn!(
                                    "Could not download a VAAPI frame ({e}); switching to software decoding"
                                );
                                reopen_in_software = true;
                                continue 'outer;
                            }
                        };

                        match send_frame(
                            &mut scaler,
                            target,
                            frame,
                            &mut rgb_frame,
                            &mut recycle_rx,
                            &recycle_tx,
                            &tx,
                        ) {
                            FrameSent::Sent => last_sent_time = target_time,
                            FrameSent::Dropped => {}
                            FrameSent::Closed => break 'outer,
                        }
                    }
                }
                // Flush decoder at end of stream
                if stream.decoder.send_eof().is_ok() {
                    let mut decoded = ffmpeg::frame::Video::empty();
                    while stream.decoder.receive_frame(&mut decoded).is_ok() {
                        let Ok(frame) = system_frame(&decoded, &mut system_buffer) else {
                            continue;
                        };
                        if let FrameSent::Closed = send_frame(
                            &mut scaler,
                            target,
                            frame,
                            &mut rgb_frame,
                            &mut recycle_rx,
                            &recycle_tx,
                            &tx,
                        ) {
                            break 'outer;
                        }
                    }
                }
            }

            info!("ffmpeg-next local decoder exited");
        });

        Ok(())
    }

    /// Streams a remote canvas video through an ffmpeg subprocess as
    /// `CanvasVideoFrame`s. While `visible_rx` reads false the subprocess is
    /// stopped entirely, and a fresh one starts when the wallpaper is
    /// visible again - canvases are short loops, so restarting from the top
    /// isn't noticeable, and it avoids holding an idle network stream open.
    pub async fn run_decoder(
        url: String,
        tx: Sender<Event>,
        mut cancel_rx: tokio::sync::watch::Receiver<bool>,
        mut visible_rx: tokio::sync::watch::Receiver<bool>,
        mut recycle_rx: tokio::sync::mpsc::Receiver<Vec<u8>>,
        recycle_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
    ) -> Result<()> {
        // Validate URL before passing to FFmpeg to prevent command injection/arbitrary file reads
        let parsed_url = match url::Url::parse(&url) {
            Ok(u) => u,
            Err(e) => {
                warn!("Invalid video URL provided: {}. Error: {}", url, e);
                return Ok(());
            }
        };

        let scheme = parsed_url.scheme();
        if scheme != "http" && scheme != "https" {
            warn!("Security violation: Unsupported video URL scheme '{}'. Only http/https are allowed.", scheme);
            return Ok(());
        }

        // SSRF guard: resolve the host ourselves and require every resolved
        // address to be publicly routable before handing the URL to ffmpeg.
        // The URL originates from the (unauthenticated) canvas proxy, so it
        // must not be able to point ffmpeg at localhost or the local network.
        //
        // Known tradeoff, accepted for V1: ffmpeg re-resolves the hostname
        // itself, so unlike `fetch_album_art` (which pins the vetted IP via
        // `ClientBuilder::resolve`) this check can be raced by a DNS rebind
        // between our lookup and ffmpeg's. Requiring *all* addresses to be
        // safe (not just one) at least closes the mixed-record variant. A
        // full pin needs `-headers Host:` + TLS SNI plumbing; deferred.
        let Some(host) = parsed_url.host_str() else {
            warn!("Security violation: video URL has no host: {}", url);
            return Ok(());
        };
        let port = parsed_url.port_or_known_default().unwrap_or(443);
        let resolved: Vec<std::net::SocketAddr> = match tokio::net::lookup_host((host, port)).await
        {
            Ok(addrs) => addrs.collect(),
            Err(e) => {
                warn!("Could not resolve video URL host '{}': {}", host, e);
                return Ok(());
            }
        };
        if resolved.is_empty()
            || !resolved
                .iter()
                .all(|addr| crate::modules::utils::is_safe_ip(addr.ip()))
        {
            warn!(
                "Security violation: video URL host '{}' resolves to a non-public address (SSRF protection)",
                host
            );
            return Ok(());
        }

        let safe_url = parsed_url.to_string();

        // Runtime check to verify FFmpeg is available before trying to decode
        let ffmpeg_path = match crate::modules::utils::resolve_binary("ffmpeg") {
            Some(path) => path,
            None => {
                warn!("Security violation/Missing dependency: FFmpeg not found in trusted PATH! Video backgrounds will not play.");
                return Ok(());
            }
        };

        if Command::new(&ffmpeg_path)
            .arg("-version")
            .output()
            .await
            .is_err()
        {
            warn!("FFmpeg failed to execute. Video backgrounds will not play.");
            return Ok(());
        }

        info!("Starting FFmpeg video decoder for: {}", safe_url);

        let width = 540;
        let height = 960;
        let frame_size = width * height * 4;

        'session: loop {
            // Wait out any period where the wallpaper is hidden before
            // (re)starting ffmpeg. Err on either channel means its sender
            // is gone: treat that as a cancel.
            while !*visible_rx.borrow_and_update() {
                if *cancel_rx.borrow() {
                    break 'session;
                }
                tokio::select! {
                    res = visible_rx.changed() => if res.is_err() { break 'session; },
                    res = cancel_rx.changed() => {
                        if res.is_err() || *cancel_rx.borrow() {
                            break 'session;
                        }
                    }
                }
            }
            if *cancel_rx.borrow() {
                break;
            }

            let mut child = Command::new(&ffmpeg_path)
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-protocol_whitelist",
                    "http,https,tcp,tls,crypto",
                    "-re", // Read input at native frame rate so we don't peg the CPU!
                    "-stream_loop",
                    "-1", // Loop the video stream infinitely
                    "-i",
                    &safe_url,
                    // Scale and crop seamlessly to ensure it fits the 9:16 Canvas perfectly
                    "-vf",
                    "scale=540:960:force_original_aspect_ratio=increase,crop=540:960",
                    "-f",
                    "rawvideo",
                    "-pix_fmt",
                    "rgba",
                    "-r",
                    "30", // Lock output to 30fps
                    "-",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .kill_on_drop(true) // Ensure the FFmpeg process dies instantly if the task is dropped
                .spawn()?;

            let mut stdout = child
                .stdout
                .take()
                .ok_or_else(|| anyhow::anyhow!("Failed to open ffmpeg stdout"))?;

            loop {
                let mut buffer = recycle_rx
                    .try_recv()
                    .unwrap_or_else(|_| vec![0u8; frame_size]);

                if buffer.len() != frame_size {
                    buffer.resize(frame_size, 0);
                }
                tokio::select! {
                    res = cancel_rx.changed() => {
                        // Err means the cancel sender was dropped without signalling
                        // (e.g. the MPRIS watcher exited). Treat it as a cancel: if we
                        // looped instead, changed() would resolve instantly on every
                        // iteration - spinning a core - and each resolution would
                        // cancel read_exact() mid-frame, losing partial reads and
                        // shearing the raw video stream out of frame alignment.
                        if res.is_err() || *cancel_rx.borrow() {
                            info!("Cancelling video stream playback");
                            break 'session;
                        }
                    }
                    res = visible_rx.changed() => {
                        // Same reasoning as above for Err. Dropping `child`
                        // (kill_on_drop) stops ffmpeg; a partial frame read
                        // is discarded with it, so the next session starts
                        // frame-aligned.
                        if res.is_err() {
                            break 'session;
                        }
                        if !*visible_rx.borrow() {
                            info!("Wallpaper hidden; stopping canvas video stream");
                            continue 'session;
                        }
                    }
                    result = stdout.read_exact(&mut buffer) => {
                        match result {
                            Ok(_) => {
                                if let Some(img) = image::RgbaImage::from_raw(width as u32, height as u32, buffer) {
                                    let pooled_img = Box::new(PooledImage::new(img, recycle_tx.clone()));
                                    match tx.try_send(Event::CanvasVideoFrame(pooled_img)) {
                                        Ok(_) => {}
                                        Err(tokio::sync::mpsc::error::TrySendError::Full(Event::CanvasVideoFrame(dropped))) => {
                                            warn!("Renderer busy, dropping video frame to prevent memory bloat");
                                            let _ = recycle_tx.try_send(dropped.into_raw());
                                        }
                                        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => break 'session,
                                        _ => {}
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("FFmpeg stream ended or errored: {}", e);
                                break 'session;
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests;
