//! VAAPI hardware decoding for the local video decoder.
//!
//! All the FFI lives here, behind a small safe surface: open a device,
//! attach it to a decoder context before that context is opened, and copy
//! decoded GPU frames back to system memory for conversion. Every step can
//! fail (no GPU, no driver, an ffmpeg built without VAAPI, a codec or
//! profile the GPU can't decode), and the decode loop falls back to
//! software decoding whenever one does.

use ffmpeg::ffi;
use ffmpeg_next as ffmpeg;
use std::ptr;

/// A VAAPI device context (`AVHWDeviceContext`), opened on the default DRM
/// render node. One is shared by every reopen of the same video.
pub(crate) struct VaapiDevice(*mut ffi::AVBufferRef);

impl VaapiDevice {
    /// Opens the default VAAPI device. Errors when there is no usable GPU
    /// or driver, or when ffmpeg was built without VAAPI support.
    pub(crate) fn open() -> Result<Self, ffmpeg::Error> {
        let mut device = ptr::null_mut();
        // SAFETY: `device` is a valid out-pointer; null device name, options
        // and flags ask ffmpeg for its default VAAPI device.
        let ret = unsafe {
            ffi::av_hwdevice_ctx_create(
                &mut device,
                ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI,
                ptr::null(),
                ptr::null_mut(),
                0,
            )
        };
        if ret < 0 || device.is_null() {
            return Err(ffmpeg::Error::from(ret.min(-1)));
        }
        Ok(Self(device))
    }
}

impl Drop for VaapiDevice {
    fn drop(&mut self) {
        // SAFETY: `self.0` is the reference created in `open`, released
        // exactly once here. Decoder contexts hold their own references.
        unsafe { ffi::av_buffer_unref(&mut self.0) };
    }
}

/// Whether `codec` has a VAAPI decode path that works with a device
/// context. Only says the codec *can* use VAAPI; whether this GPU supports
/// the stream's profile is decided per stream in `select_vaapi_format`.
pub(crate) fn codec_supports_vaapi(codec: &ffmpeg::Codec) -> bool {
    for index in 0.. {
        // SAFETY: `codec` wraps a valid AVCodec; avcodec_get_hw_config
        // returns null past the last config.
        let config = unsafe { ffi::avcodec_get_hw_config(codec.as_ptr(), index) };
        if config.is_null() {
            return false;
        }
        // SAFETY: non-null configs point to static, immutable data.
        let config = unsafe { &*config };
        if config.device_type == ffi::AVHWDeviceType::AV_HWDEVICE_TYPE_VAAPI
            && config.pix_fmt == ffi::AVPixelFormat::AV_PIX_FMT_VAAPI
            && config.methods & ffi::AV_CODEC_HW_CONFIG_METHOD_HW_DEVICE_CTX as i32 != 0
        {
            return true;
        }
    }
    false
}

/// Attaches `device` to a decoder context that has not been opened yet, so
/// that frames the GPU can decode come out as VAAPI surfaces.
pub(crate) fn attach(
    context: &mut ffmpeg::codec::Context,
    device: &VaapiDevice,
) -> Result<(), ffmpeg::Error> {
    // SAFETY: `device.0` is a live device reference; the new reference is
    // owned by the codec context from here on and released when the context
    // is freed. The context isn't open yet, so setting these fields is
    // allowed.
    unsafe {
        let reference = ffi::av_buffer_ref(device.0);
        if reference.is_null() {
            return Err(ffmpeg::Error::Other { errno: ffi::ENOMEM });
        }
        let ctx = context.as_mut_ptr();
        ffi::av_buffer_unref(&mut (*ctx).hw_device_ctx);
        (*ctx).hw_device_ctx = reference;
        (*ctx).get_format = Some(select_vaapi_format);
    }
    Ok(())
}

/// `AVCodecContext::get_format` callback: picks VAAPI when the decoder
/// offers it for this stream, and otherwise the first software format, so
/// a stream the GPU can't handle (an unsupported profile, say) still
/// decodes - in software - instead of failing.
unsafe extern "C" fn select_vaapi_format(
    _ctx: *mut ffi::AVCodecContext,
    formats: *const ffi::AVPixelFormat,
) -> ffi::AVPixelFormat {
    let mut software = ffi::AVPixelFormat::AV_PIX_FMT_NONE;
    let mut cursor = formats;
    // SAFETY: ffmpeg passes a list terminated by AV_PIX_FMT_NONE.
    unsafe {
        while *cursor != ffi::AVPixelFormat::AV_PIX_FMT_NONE {
            let format = *cursor;
            if format == ffi::AVPixelFormat::AV_PIX_FMT_VAAPI {
                return format;
            }
            if software == ffi::AVPixelFormat::AV_PIX_FMT_NONE && !is_hwaccel_format(format) {
                software = format;
            }
            cursor = cursor.add(1);
        }
    }
    software
}

fn is_hwaccel_format(format: ffi::AVPixelFormat) -> bool {
    // SAFETY: av_pix_fmt_desc_get returns null or a static descriptor.
    let descriptor = unsafe { ffi::av_pix_fmt_desc_get(format) };
    !descriptor.is_null()
        && unsafe { (*descriptor).flags } & ffi::AV_PIX_FMT_FLAG_HWACCEL as u64 != 0
}

/// Whether `frame` is a VAAPI surface (still in GPU memory).
pub(crate) fn is_vaapi_frame(frame: &ffmpeg::frame::Video) -> bool {
    // SAFETY: reads the format field of a valid AVFrame.
    unsafe { (*frame.as_ptr()).format == ffi::AVPixelFormat::AV_PIX_FMT_VAAPI as i32 }
}

/// Copies a VAAPI surface into `system` (NV12, or P010 for 10-bit video),
/// along with the colour properties the RGB conversion needs.
///
/// `system` is reset and reallocated by ffmpeg on every call - the
/// pattern ffmpeg's own hw_decode example uses. (The allocator hands back
/// the block freed by the reset, so this doesn't fault in fresh pages.)
/// Only the colour fields are copied, deliberately not
/// `av_frame_copy_props`: that appends side data without clearing it,
/// which would grow a reused frame on every call.
pub(crate) fn download(
    gpu: &ffmpeg::frame::Video,
    system: &mut ffmpeg::frame::Video,
) -> Result<(), ffmpeg::Error> {
    // SAFETY: both frames are valid AVFrames, and `system` is exclusively
    // ours. After av_frame_unref, `system` holds no buffers, so
    // av_hwframe_transfer_data allocates its own (in the surface's
    // download format) and sets its size; on failure it's left empty.
    unsafe {
        let dst = system.as_mut_ptr();
        let src = gpu.as_ptr();
        ffi::av_frame_unref(dst);
        let ret = ffi::av_hwframe_transfer_data(dst, src, 0);
        if ret < 0 {
            ffi::av_frame_unref(dst);
            return Err(ffmpeg::Error::from(ret));
        }
        (*dst).colorspace = (*src).colorspace;
        (*dst).color_range = (*src).color_range;
        (*dst).color_primaries = (*src).color_primaries;
        (*dst).color_trc = (*src).color_trc;
        (*dst).chroma_location = (*src).chroma_location;
        (*dst).pts = (*src).pts;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_formats_are_not_hwaccel() {
        assert!(!is_hwaccel_format(ffi::AVPixelFormat::AV_PIX_FMT_YUV420P));
        assert!(!is_hwaccel_format(ffi::AVPixelFormat::AV_PIX_FMT_NV12));
        assert!(is_hwaccel_format(ffi::AVPixelFormat::AV_PIX_FMT_VAAPI));
    }

    #[test]
    fn get_format_prefers_vaapi_then_software() {
        use ffi::AVPixelFormat::*;
        let offered = [AV_PIX_FMT_VAAPI, AV_PIX_FMT_YUV420P, AV_PIX_FMT_NONE];
        // SAFETY: a NONE-terminated list, as ffmpeg passes it; the context
        // argument is unused.
        let chosen = unsafe { select_vaapi_format(ptr::null_mut(), offered.as_ptr()) };
        assert_eq!(chosen, AV_PIX_FMT_VAAPI);

        // No VAAPI on offer (e.g. an unsupported profile): the first
        // software format, skipping other hardware formats.
        let offered = [
            AV_PIX_FMT_CUDA,
            AV_PIX_FMT_NV12,
            AV_PIX_FMT_YUV420P,
            AV_PIX_FMT_NONE,
        ];
        let chosen = unsafe { select_vaapi_format(ptr::null_mut(), offered.as_ptr()) };
        assert_eq!(chosen, AV_PIX_FMT_NV12);
    }

    #[test]
    fn common_codecs_report_vaapi_support_consistently() {
        ffmpeg::init().unwrap();
        // mpeg4 has a VAAPI hwaccel in ffmpeg builds that enable VAAPI and
        // none otherwise - either way the query must not crash, and a
        // codec with no hw configs at all must report false.
        if let Some(codec) = ffmpeg::decoder::find(ffmpeg::codec::Id::MPEG4) {
            let _ = codec_supports_vaapi(&codec);
        }
        // rawvideo is built into every configuration (unlike, say, PNG,
        // which needs zlib - absent from the static build) and has no
        // hardware configs.
        let raw = ffmpeg::decoder::find(ffmpeg::codec::Id::RAWVIDEO).expect("rawvideo decoder");
        assert!(!codec_supports_vaapi(&raw));
    }

    #[test]
    fn opening_a_device_fails_cleanly_without_a_gpu() {
        ffmpeg::init().unwrap();
        // CI machines have no render node; on a desktop this may succeed.
        // Either outcome is fine - what matters is no crash or leak.
        match VaapiDevice::open() {
            Ok(device) => drop(device),
            Err(e) => assert!(!e.to_string().is_empty()),
        }
    }
}
