use image::codecs::ico::IcoEncoder;
use image::codecs::jpeg::JpegEncoder;
use image::codecs::png::PngEncoder;
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageEncoder, ImageFormat, Rgb, RgbImage, RgbaImage};
use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use tracing::{debug, info};

pub struct ImageConverter;

impl ImageConverter {
    pub fn new() -> Self {
        Self
    }

    /// Composite an RGBA image onto a solid background color (for non-alpha target formats like JPEG).
    pub fn composite_over_background(rgba: &RgbaImage, bg: (u8, u8, u8)) -> RgbImage {
        let (width, height) = rgba.dimensions();
        let mut rgb_img = RgbImage::new(width, height);

        for (x, y, pixel) in rgba.enumerate_pixels() {
            let [r, g, b, a] = pixel.0;
            if a == 255 {
                rgb_img.put_pixel(x, y, Rgb([r, g, b]));
            } else if a == 0 {
                rgb_img.put_pixel(x, y, Rgb([bg.0, bg.1, bg.2]));
            } else {
                let alpha = a as f32 / 255.0;
                let inv = 1.0 - alpha;
                let comp_r = (r as f32 * alpha + bg.0 as f32 * inv).round() as u8;
                let comp_g = (g as f32 * alpha + bg.1 as f32 * inv).round() as u8;
                let comp_b = (b as f32 * alpha + bg.2 as f32 * inv).round() as u8;
                rgb_img.put_pixel(x, y, Rgb([comp_r, comp_g, comp_b]));
            }
        }

        rgb_img
    }

    /// Encode ICO format, generating appropriate icon mip-levels.
    pub fn save_as_ico(
        img: &DynamicImage,
        output_path: &Path,
        target_sizes: &[u32],
    ) -> Result<(), ConversionError> {
        let file = File::create(output_path).map_err(|e| ConversionError::IoError(e.to_string()))?;
        let writer = BufWriter::new(file);
        let encoder = IcoEncoder::new(writer);

        let (orig_w, orig_h) = img.dimensions();
        // If image is square or small, pick matching standard sizes up to 256
        let max_dim = orig_w.max(orig_h);
        let applicable_sizes: Vec<u32> = target_sizes
            .iter()
            .copied()
            .filter(|&sz| sz <= max_dim || sz <= 256)
            .collect();

        let sizes_to_use = if applicable_sizes.is_empty() {
            vec![32, 48, 64, 128, 256]
        } else {
            applicable_sizes
        };

        // For ICO, we can encode the best resolution RGBA representation
        let largest_size = sizes_to_use.iter().copied().max().unwrap_or(256).min(256);
        let resized = img.resize_exact(largest_size, largest_size, FilterType::Lanczos3);
        let rgba = resized.to_rgba8();

        encoder
            .write_image(
                rgba.as_raw(),
                largest_size,
                largest_size,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| ConversionError::ProcessingError(format!("ICO encoding failed: {}", e)))?;

        Ok(())
    }
}

impl Converter for ImageConverter {
    fn id(&self) -> &'static str {
        "orbit.converter.image"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        let raster_formats = [
            FileFormat::Png,
            FileFormat::Jpg,
            FileFormat::Webp,
            FileFormat::Ico,
            FileFormat::Bmp,
            FileFormat::Tiff,
            FileFormat::Gif,
            FileFormat::Tga,
            FileFormat::Ppm,
        ];
        from != to && raster_formats.contains(&from) && raster_formats.contains(&to)
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

        debug!("Reading image from {}", input.display());
        let dyn_img = image::open(input)
            .map_err(|e| ConversionError::InvalidInput(format!("Could not read image: {}", e)))?;

        if let Some(cb) = progress {
            cb(0.5);
        }

        let target_format = FileFormat::from_path(output).ok_or_else(|| {
            ConversionError::ProcessingError(format!(
                "Could not determine target format from output path {}",
                output.display()
            ))
        })?;

        self.convert_dynamic_image(dyn_img, output, target_format, options, progress)
    }
}

impl ImageConverter {
    pub fn convert_dynamic_image(
        &self,
        dyn_img: DynamicImage,
        output: &Path,
        target_format: FileFormat,
        options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        let file = File::create(output).map_err(|e| ConversionError::IoError(e.to_string()))?;
        let mut writer = BufWriter::new(file);

        match target_format {
            FileFormat::Jpg => {
                // JPEG does not support transparency. Composite over background.
                let rgb_img = if dyn_img.color().has_alpha() {
                    Self::composite_over_background(&dyn_img.to_rgba8(), options.background_color)
                } else {
                    dyn_img.to_rgb8()
                };

                let mut encoder = JpegEncoder::new_with_quality(&mut writer, options.jpeg_quality);
                encoder
                    .encode_image(&rgb_img)
                    .map_err(|e| ConversionError::ProcessingError(format!("JPEG encoding failed: {}", e)))?;
            }
            FileFormat::Png => {
                let encoder = PngEncoder::new(&mut writer);
                dyn_img
                    .write_with_encoder(encoder)
                    .map_err(|e| ConversionError::ProcessingError(format!("PNG encoding failed: {}", e)))?;
            }
            FileFormat::Webp => {
                let encoder = WebPEncoder::new_lossless(&mut writer);
                let rgba = dyn_img.to_rgba8();
                encoder
                    .write_image(
                        rgba.as_raw(),
                        rgba.width(),
                        rgba.height(),
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|e| ConversionError::ProcessingError(format!("WebP encoding failed: {}", e)))?;
            }
            FileFormat::Ico => {
                drop(writer); // Close writer so save_as_ico can open file cleanly
                Self::save_as_ico(&dyn_img, output, &options.ico_sizes)?;
            }
            FileFormat::Bmp => {
                dyn_img
                    .save_with_format(output, ImageFormat::Bmp)
                    .map_err(|e| ConversionError::ProcessingError(format!("BMP encoding failed: {}", e)))?;
            }
            FileFormat::Tiff => {
                dyn_img
                    .save_with_format(output, ImageFormat::Tiff)
                    .map_err(|e| ConversionError::ProcessingError(format!("TIFF encoding failed: {}", e)))?;
            }
            FileFormat::Gif => {
                dyn_img
                    .save_with_format(output, ImageFormat::Gif)
                    .map_err(|e| ConversionError::ProcessingError(format!("GIF encoding failed: {}", e)))?;
            }
            FileFormat::Tga => {
                dyn_img
                    .save_with_format(output, ImageFormat::Tga)
                    .map_err(|e| ConversionError::ProcessingError(format!("TGA encoding failed: {}", e)))?;
            }
            FileFormat::Ppm => {
                dyn_img
                    .save_with_format(output, ImageFormat::Pnm)
                    .map_err(|e| ConversionError::ProcessingError(format!("PPM encoding failed: {}", e)))?;
            }
            other => {
                return Err(ConversionError::UnsupportedConversion {
                    from: FileFormat::Png,
                    to: other,
                });
            }
        }

        if let Some(cb) = progress {
            cb(1.0);
        }

        info!("Successfully written converted image to {}", output.display());
        Ok(())
    }
}
