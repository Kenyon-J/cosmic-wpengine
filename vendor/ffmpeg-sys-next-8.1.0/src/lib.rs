#![allow(non_upper_case_globals)]
#![allow(non_camel_case_types)]
#![allow(non_snake_case)]
#![allow(clippy::approx_constant)]
#![allow(clippy::missing_safety_doc)]
#![allow(clippy::redundant_static_lifetimes)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::ptr_offset_with_cast)]
#![allow(clippy::useless_transmute)]
#![allow(unpredictable_function_pointer_comparisons)]
#![allow(unnecessary_transmutes)]

extern crate libc;

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[macro_use]
mod avutil;
pub use avutil::*;

// Polyfills for missing variants in newer FFmpeg versions
pub const AV_CODEC_ID_V410: AVCodecID = AVCodecID::AV_CODEC_ID_V210;
pub const AV_CODEC_ID_V308: AVCodecID = AVCodecID::AV_CODEC_ID_MV30;
pub const AV_CODEC_ID_V408: AVCodecID = AVCodecID::AV_CODEC_ID_RV40;
