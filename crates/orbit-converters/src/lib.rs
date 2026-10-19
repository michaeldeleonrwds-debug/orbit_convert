pub mod data;
pub mod extended;
pub mod ffmpeg;
pub mod images;
pub mod svg;

use orbit_core::registry::ConversionRegistry;
use std::sync::Arc;

pub fn register_all_converters(registry: &mut ConversionRegistry) {
    registry.register(Arc::new(images::ImageConverter::new()));
    registry.register(Arc::new(extended::ExtendedImageConverter::new()));
    registry.register(Arc::new(svg::SvgConverter::new()));
    registry.register(Arc::new(ffmpeg::FfmpegConverter::new()));
    registry.register(Arc::new(data::DataConverter::new()));
}
