use crate::formats::FileFormat;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionOptions {
    /// JPEG quality factor (1-100). Default is 90.
    pub jpeg_quality: u8,
    /// WebP quality factor (0.0 - 100.0). Default is 85.0.
    pub webp_quality: f32,
    /// WebP lossless encoding flag. Default is false.
    pub webp_lossless: bool,
    /// SVG scaling factor. 1.0 means original viewBox/dimensions.
    pub svg_scale: f32,
    /// Explicit SVG target width if specified.
    pub svg_target_width: Option<u32>,
    /// Explicit SVG target height if specified.
    pub svg_target_height: Option<u32>,
    /// RGB color used to composite transparent alpha channels when converting to non-alpha formats like JPEG.
    /// Default is pure white: (255, 255, 255).
    pub background_color: (u8, u8, u8),
    /// Target icon sizes when generating ICO. Default is [16, 32, 48, 64, 128, 256].
    pub ico_sizes: Vec<u32>,
}

impl Default for ConversionOptions {
    fn default() -> Self {
        Self {
            jpeg_quality: 90,
            webp_quality: 85.0,
            webp_lossless: false,
            svg_scale: 1.0,
            svg_target_width: None,
            svg_target_height: None,
            background_color: (255, 255, 255),
            ico_sizes: vec![16, 32, 48, 64, 128, 256],
        }
    }
}

#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Invalid or corrupted input file: {0}")]
    InvalidInput(String),

    #[error("Unsupported conversion from {from} to {to}")]
    UnsupportedConversion {
        from: FileFormat,
        to: FileFormat,
    },

    #[error("Conversion processing failed: {0}")]
    ProcessingError(String),

    #[error("Filesystem I/O error: {0}")]
    IoError(String),

    #[error("Conversion was cancelled")]
    Cancelled,
}

pub type ProgressReporter<'a> = Option<&'a dyn Fn(f32)>;

/// Trait implemented by conversion adapters (image-rs, resvg, future ffmpeg, etc.).
pub trait Converter: Send + Sync {
    /// Unique identifier for this converter adapter.
    fn id(&self) -> &'static str;

    /// Returns true if this converter can transform `from` into `to`.
    fn supports(&self, from: FileFormat, to: FileFormat) -> bool;

    /// Execute the file conversion.
    fn convert(
        &self,
        input: &Path,
        output: &Path,
        options: &ConversionOptions,
        progress: ProgressReporter<'_>,
    ) -> Result<(), ConversionError>;
}

/// Registry mapping source formats to target formats and their responsible converter.
#[derive(Default, Clone)]
pub struct ConversionRegistry {
    converters: Vec<Arc<dyn Converter>>,
    routes: HashMap<FileFormat, HashSet<FileFormat>>,
}

impl ConversionRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a converter implementation.
    pub fn register(&mut self, converter: Arc<dyn Converter>) {
        // Query known formats
        let all_formats = [
            FileFormat::Svg,
            FileFormat::Png,
            FileFormat::Jpg,
            FileFormat::Webp,
            FileFormat::Ico,
            FileFormat::Bmp,
            FileFormat::Tiff,
            FileFormat::Gif,
            FileFormat::Mp4,
            FileFormat::Mov,
            FileFormat::Mkv,
            FileFormat::Avi,
            FileFormat::Webm,
            FileFormat::Mp3,
            FileFormat::Wav,
            FileFormat::Flac,
            FileFormat::Aac,
            FileFormat::M4a,
            FileFormat::Ogg,
            FileFormat::Csv,
            FileFormat::Xlsx,
            FileFormat::Json,
            FileFormat::Tsv,
            FileFormat::Pdf,
            FileFormat::Docx,
            FileFormat::Txt,
            FileFormat::Html,
        ];

        for &from in &all_formats {
            for &to in &all_formats {
                if from != to && converter.supports(from, to) {
                    self.routes.entry(from).or_default().insert(to);
                }
            }
        }

        self.converters.push(converter);
    }

    /// Retrieve all available target formats for a given source format.
    /// Sorts them according to a preferred radial layout ordering.
    pub fn get_available_targets(&self, source: FileFormat) -> Vec<FileFormat> {
        let mut targets: Vec<FileFormat> = self
            .routes
            .get(&source)
            .map(|set| set.iter().copied().collect())
            .unwrap_or_default();

        targets.sort_by_key(|fmt| match fmt {
            FileFormat::Png => 1,
            FileFormat::Jpg => 2,
            FileFormat::Webp => 3,
            FileFormat::Ico => 4,
            FileFormat::Svg => 5,
            FileFormat::Bmp => 6,
            FileFormat::Tiff => 7,
            FileFormat::Gif => 8,
            FileFormat::Mp3 => 10,
            FileFormat::Wav => 11,
            FileFormat::Aac => 12,
            FileFormat::Flac => 13,
            FileFormat::M4a => 14,
            FileFormat::Ogg => 15,
            FileFormat::Mp4 => 20,
            FileFormat::Webm => 21,
            FileFormat::Mov => 22,
            FileFormat::Mkv => 23,
            FileFormat::Avi => 24,
            FileFormat::Json => 30,
            FileFormat::Csv => 31,
            FileFormat::Xlsx => 32,
            FileFormat::Tsv => 33,
            FileFormat::Pdf => 40,
            FileFormat::Docx => 41,
            FileFormat::Txt => 42,
            FileFormat::Html => 43,
        });

        targets
    }

    /// Find an active converter that supports transforming `source` to `target`.
    pub fn find_converter(
        &self,
        source: FileFormat,
        target: FileFormat,
    ) -> Option<Arc<dyn Converter>> {
        self.converters
            .iter()
            .find(|c| c.supports(source, target))
            .cloned()
    }
}
