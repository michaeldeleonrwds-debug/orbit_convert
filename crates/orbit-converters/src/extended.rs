use image::DynamicImage;
use orbit_core::formats::FileFormat;
use orbit_core::registry::{ConversionError, ConversionOptions, Converter, ProgressReporter};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub struct ExtendedImageConverter {
    ffmpeg_available: bool,
    magick_available: bool,
}

impl ExtendedImageConverter {
    pub fn new() -> Self {
        let ffmpeg_available = Self::probe_command("ffmpeg");
        let magick_available = Self::probe_command("magick");

        Self {
            ffmpeg_available,
            magick_available,
        }
    }

    fn probe_command(cmd_name: &str) -> bool {
        let mut cmd = Command::new(cmd_name);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        cmd.arg("-version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
    }

    /// Extract embedded full-resolution JPEG stream from camera RAW files (CR2, NEF, ARW, DNG, etc.).
    /// Most digital cameras embed a high-resolution JPEG inside the RAW container IFD tags.
    pub fn extract_embedded_raw_jpeg(input: &Path) -> Option<DynamicImage> {
        let file = File::open(input).ok()?;
        let metadata = file.metadata().ok()?;
        // Read up to 8MB or file size to locate the full-res preview JPEG
        let max_read = (metadata.len() as usize).min(8 * 1024 * 1024);
        let mut buffer = Vec::with_capacity(max_read);
        let mut handle = file.take(max_read as u64);
        handle.read_to_end(&mut buffer).ok()?;

        // Search for JPEG SOI marker (0xFF 0xD8 0xFF)
        let mut offset = 0;
        let mut best_img: Option<DynamicImage> = None;
        let mut best_area = 0;

        while offset + 3 < buffer.len() {
            if buffer[offset] == 0xFF && buffer[offset + 1] == 0xD8 && buffer[offset + 2] == 0xFF {
                // Found JPEG SOI. Find matching EOI marker (0xFF 0xD9)
                let jpeg_start = offset;
                let mut eoi = jpeg_start + 2;
                while eoi + 1 < buffer.len() {
                    if buffer[eoi] == 0xFF && buffer[eoi + 1] == 0xD9 {
                        let jpeg_slice = &buffer[jpeg_start..=eoi + 1];
                        if let Ok(dyn_img) = image::load_from_memory(jpeg_slice) {
                            let (w, h) = (dyn_img.width(), dyn_img.height());
                            let area = w * h;
                            // Prefer larger full-size previews over small thumbnail previews
                            if area > best_area {
                                best_area = area;
                                best_img = Some(dyn_img);
                            }
                        }
                        break;
                    }
                    eoi += 1;
                }
            }
            offset += 1;
        }

        best_img
    }

    /// Extract first cover image from a CBZ (ZIP) comic archive using pure Rust.
    pub fn extract_cbz_cover(input: &Path) -> Result<DynamicImage, ConversionError> {
        let file = File::open(input).map_err(|e| ConversionError::IoError(e.to_string()))?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| ConversionError::InvalidInput(format!("Invalid CBZ/ZIP archive: {}", e)))?;

        // Find first image entry
        for i in 0..archive.len() {
            let mut file_entry = archive.by_index(i).map_err(|e| {
                ConversionError::ProcessingError(format!("Failed to read archive entry: {}", e))
            })?;

            let name = file_entry.name().to_lowercase();
            if name.ends_with(".png")
                || name.ends_with(".jpg")
                || name.ends_with(".jpeg")
                || name.ends_with(".webp")
            {
                let mut img_bytes = Vec::new();
                file_entry.read_to_end(&mut img_bytes).map_err(|e| {
                    ConversionError::ProcessingError(format!("Failed to unpack cover: {}", e))
                })?;

                return image::load_from_memory(&img_bytes).map_err(|e| {
                    ConversionError::ProcessingError(format!("Corrupt cover image: {}", e))
                });
            }
        }

        Err(ConversionError::InvalidInput(
            "No compatible images found inside CBZ comic archive".into(),
        ))
    }
}

impl Converter for ExtendedImageConverter {
    fn id(&self) -> &'static str {
        "orbit.converter.extended"
    }

    fn supports(&self, from: FileFormat, to: FileFormat) -> bool {
        let writable_raster = [
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

        // Ensure target is an actual writable raster format
        if !writable_raster.contains(&to) {
            return false;
        }

        // Camera RAW formats (Decode Only)
        let is_raw = matches!(
            from,
            FileFormat::Cr2
                | FileFormat::Cr3
                | FileFormat::Crw
                | FileFormat::Nef
                | FileFormat::Nrw
                | FileFormat::Arw
                | FileFormat::Sr2
                | FileFormat::Srf
                | FileFormat::Raf
                | FileFormat::Orf
                | FileFormat::Rw2
                | FileFormat::Dng
                | FileFormat::Pef
                | FileFormat::Ptx
                | FileFormat::Rwl
                | FileFormat::Srw
                | FileFormat::X3f
                | FileFormat::Dcr
                | FileFormat::Drf
                | FileFormat::Kdc
                | FileFormat::K25
        );

        // Modern Web & Next-Gen formats
        let is_modern_web = matches!(
            from,
            FileFormat::Avif | FileFormat::Heic | FileFormat::Jxl
        );

        // Specialized, Vector & Graphic formats
        let is_specialized = matches!(
            from,
            FileFormat::Psd
                | FileFormat::Psb
                | FileFormat::Djvu
                | FileFormat::Cbr
                | FileFormat::Cbz
                | FileFormat::Odd
                | FileFormat::Dpx
                | FileFormat::Pcx
                | FileFormat::Eps
                | FileFormat::Art
                | FileFormat::Dcs
                | FileFormat::Wmz
                | FileFormat::Emz
        );

        from != to && (is_raw || is_modern_web || is_specialized)
    }

    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError> {
        let from = FileFormat::from_path(input).ok_or_else(|| {
            ConversionError::InvalidInput("Unknown input format".to_string())
        })?;
        let to = FileFormat::from_path(output).ok_or_else(|| {
            ConversionError::ProcessingError("Unknown output format".to_string())
        })?;

        if let Some(cb) = progress {
            cb(0.15);
        }

        // 1. Try CBZ extraction if comic book archive
        if from == FileFormat::Cbz {
            match Self::extract_cbz_cover(input) {
                Ok(dyn_img) => {
                    return crate::images::ImageConverter::new().convert_dynamic_image(
                        dyn_img, output, to, options, progress,
                    );
                }
                Err(e) => {
                    debug!("CBZ extraction failed: {}", e);
                }
            }
        }

        // 2. Try embedded RAW JPEG extraction for camera RAW formats
        if from.is_camera_raw() {
            if let Some(dyn_img) = Self::extract_embedded_raw_jpeg(input) {
                info!(
                    source = %input.display(),
                    target = %to,
                    "Successfully extracted embedded camera RAW image stream"
                );
                return crate::images::ImageConverter::new().convert_dynamic_image(
                    dyn_img, output, to, options, progress,
                );
            }
        }

        // 3. Try external ImageMagick if available
        if self.magick_available {
            let mut cmd = Command::new("magick");
            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.arg("convert")
                .arg(input)
                .arg(output);

            let status = cmd.status().map_err(|e| {
                ConversionError::ProcessingError(format!("Failed to spawn magick: {}", e))
            })?;

            if status.success() {
                if let Some(cb) = progress {
                    cb(1.0);
                }
                return Ok(());
            }
        }

        // 4. Try external FFmpeg if available
        if self.ffmpeg_available {
            let mut cmd = Command::new("ffmpeg");
            #[cfg(windows)]
            cmd.creation_flags(CREATE_NO_WINDOW);
            cmd.args(&["-y", "-v", "error"])
                .arg("-i")
                .arg(input)
                .arg(output);

            let status = cmd.status().map_err(|e| {
                ConversionError::ProcessingError(format!("Failed to spawn ffmpeg: {}", e))
            })?;

            if status.success() {
                if let Some(cb) = progress {
                    cb(1.0);
                }
                return Ok(());
            }
        }

        // Fallback: If external codecs are required
        Err(ConversionError::ProcessingError(format!(
            "Conversion for {} requires ImageMagick or FFmpeg. Please install them on your system PATH to decode this format.",
            from.display_label()
        )))
    }
}
