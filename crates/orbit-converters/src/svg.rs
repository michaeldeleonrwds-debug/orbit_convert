use crate::images::ImageConverter;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::webp::WebPEncoder;
use image::{DynamicImage, ImageEncoder, RgbaImage};
use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;
use tracing::{debug, info};

pub struct SvgConverter;

impl SvgConverter {
    pub fn new() -> Self {
        Self
    }

    /// Rasterizes an SVG file into a `tiny_skia::Pixmap` (RGBA buffer).
    pub fn rasterize(
        svg_path: &Path,
        options: &ConversionOptions,
    ) -> Result<tiny_skia::Pixmap, ConversionError> {
        let svg_bytes = fs::read(svg_path).map_err(|e| {
            ConversionError::InvalidInput(format!("Failed to read SVG file: {}", e))
        })?;

        let mut opt = usvg::Options::default();
        // Fallback font family setup
        opt.font_family = "Segoe UI".to_string();

        let tree = usvg::Tree::from_data(&svg_bytes, &opt).map_err(|e| {
            ConversionError::InvalidInput(format!("Malformed SVG XML/structure: {}", e))
        })?;

        let original_size = tree.size();
        let orig_w = original_size.width();
        let orig_h = original_size.height();

        let (target_w, target_h, scale_x, scale_y) = match (options.svg_target_width, options.svg_target_height) {
            (Some(w), Some(h)) => {
                let sx = w as f32 / orig_w;
                let sy = h as f32 / orig_h;
                (w, h, sx, sy)
            }
            (Some(w), None) => {
                let ratio = orig_h / orig_w;
                let h = ((w as f32) * ratio).round().max(1.0) as u32;
                let sx = w as f32 / orig_w;
                (w, h, sx, sx)
            }
            (None, Some(h)) => {
                let ratio = orig_w / orig_h;
                let w = ((h as f32) * ratio).round().max(1.0) as u32;
                let sy = h as f32 / orig_h;
                (w, h, sy, sy)
            }
            (None, None) => {
                let scale = options.svg_scale.max(0.1);
                let w = (orig_w * scale).round().max(1.0) as u32;
                let h = (orig_h * scale).round().max(1.0) as u32;
                (w, h, scale, scale)
            }
        };

        let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h).ok_or_else(|| {
            ConversionError::ProcessingError(format!(
                "Failed to allocate raster buffer for SVG dimensions {}x{}",
                target_w, target_h
            ))
        })?;

        let transform = tiny_skia::Transform::from_scale(scale_x, scale_y);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        Ok(pixmap)
    }
}

impl Converter for SvgConverter {
    fn id(&self) -> &'static str {
        "orbit.converter.svg"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        from == FileFormat::Svg
            && matches!(
                to,
                FileFormat::Png
                    | FileFormat::Jpg
                    | FileFormat::Webp
                    | FileFormat::Ico
                    | FileFormat::Bmp
                    | FileFormat::Tiff
                    | FileFormat::Gif
            )
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        if let Some(cb) = progress {
            cb(0.1);
        }

        debug!("Rasterizing SVG: {}", input.display());
        let pixmap = Self::rasterize(input, options)?;

        if let Some(cb) = progress {
            cb(0.6);
        }

        let target_format = FileFormat::from_path(output).ok_or_else(|| {
            ConversionError::ProcessingError(format!(
                "Could not determine target format from output path {}",
                output.display()
            ))
        })?;

        let width = pixmap.width();
        let height = pixmap.height();

        // Convert tiny_skia premultiplied/unmultiplied RGBA into RgbaImage
        let mut rgba_raw = Vec::with_capacity((width * height * 4) as usize);
        for pixel in pixmap.pixels() {
            // tiny_skia pixels are PremultipliedColorU8
            let demul = pixel.demultiply();
            rgba_raw.push(demul.red());
            rgba_raw.push(demul.green());
            rgba_raw.push(demul.blue());
            rgba_raw.push(demul.alpha());
        }

        let rgba_image = RgbaImage::from_raw(width, height, rgba_raw).ok_or_else(|| {
            ConversionError::ProcessingError("Failed to construct RGBA image buffer".into())
        })?;

        match target_format {
            FileFormat::Png => {
                let file = File::create(output).map_err(|e| ConversionError::IoError(e.to_string()))?;
                let mut writer = BufWriter::new(file);
                let encoder = image::codecs::png::PngEncoder::new(&mut writer);
                encoder
                    .write_image(
                        rgba_image.as_raw(),
                        width,
                        height,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| ConversionError::ProcessingError(format!("PNG encoding error: {}", e)))?;
            }
            FileFormat::Jpg => {
                let rgb_img = ImageConverter::composite_over_background(
                    &rgba_image,
                    options.background_color,
                );
                let file = File::create(output).map_err(|e| ConversionError::IoError(e.to_string()))?;
                let mut writer = BufWriter::new(file);
                let mut encoder = JpegEncoder::new_with_quality(&mut writer, options.jpeg_quality);
                encoder
                    .encode_image(&rgb_img)
                    .map_err(|e| ConversionError::ProcessingError(format!("JPEG encoding error: {}", e)))?;
            }
            FileFormat::Webp => {
                let file = File::create(output).map_err(|e| ConversionError::IoError(e.to_string()))?;
                let mut writer = BufWriter::new(file);
                let encoder = WebPEncoder::new_lossless(&mut writer);
                encoder
                    .write_image(
                        rgba_image.as_raw(),
                        width,
                        height,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| ConversionError::ProcessingError(format!("WebP encoding error: {}", e)))?;
            }
            FileFormat::Ico => {
                let dyn_img = DynamicImage::ImageRgba8(rgba_image);
                ImageConverter::save_as_ico(&dyn_img, output, &options.ico_sizes)?;
            }
            FileFormat::Bmp => {
                let dyn_img = DynamicImage::ImageRgba8(rgba_image);
                dyn_img
                    .save_with_format(output, image::ImageFormat::Bmp)
                    .map_err(|e| ConversionError::ProcessingError(format!("BMP encoding failed: {}", e)))?;
            }
            FileFormat::Tiff => {
                let dyn_img = DynamicImage::ImageRgba8(rgba_image);
                dyn_img
                    .save_with_format(output, image::ImageFormat::Tiff)
                    .map_err(|e| ConversionError::ProcessingError(format!("TIFF encoding failed: {}", e)))?;
            }
            FileFormat::Gif => {
                let dyn_img = DynamicImage::ImageRgba8(rgba_image);
                dyn_img
                    .save_with_format(output, image::ImageFormat::Gif)
                    .map_err(|e| ConversionError::ProcessingError(format!("GIF encoding failed: {}", e)))?;
            }
            _ => {
                return Err(ConversionError::UnsupportedConversion {
                    from: FileFormat::Svg,
                    to: target_format,
                });
            }
        }

        if let Some(cb) = progress {
            cb(1.0);
        }

        info!(
            "SVG converted successfully: {} -> {}",
            input.display(),
            output.display()
        );
        Ok(())
    }
}
